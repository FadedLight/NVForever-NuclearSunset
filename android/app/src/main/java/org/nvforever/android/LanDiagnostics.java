package org.nvforever.android;

import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.ApplicationInfo;
import android.os.Build;
import java.io.File;
import java.io.InputStream;
import java.net.URL;
import java.nio.channels.FileLock;
import java.nio.channels.FileChannel;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Opt-in test support. Only the launcher process starts this bounded network worker. */
final class LanDiagnostics {
    static final String DEFAULT_ENDPOINT = "http://192.168.50.116:8765/api/v1/log-bundles";
    private static LanDiagnostics instance;
    private final android.app.Application context;
    private final SharedPreferences preferences;
    private final File directory;
    private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread thread = new Thread(r, "nvf-lan-diagnostics"); thread.setDaemon(true); return thread;
    });
    private final AtomicBoolean queued = new AtomicBoolean();
    private final DiagnosticsSession session;
    private volatile String status = "Off";
    private volatile long pausedGeneration = -1;
    private final AtomicBoolean retryRequested = new AtomicBoolean();
    private final AtomicBoolean exporting = new AtomicBoolean();
    private String directAckPair = "", directAckSnapshot = "";

    static synchronized LanDiagnostics launcher(Context context) {
        if (instance == null) instance = new LanDiagnostics((android.app.Application)context.getApplicationContext());
        return instance;
    }
    static boolean available(Context context) {
        return BuildConfig.NVF_DIAGNOSTICS
            && (context.getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0;
    }
    private LanDiagnostics(android.app.Application context) {
        this.context = context;
        preferences = context.getSharedPreferences("lan-diagnostics", Context.MODE_PRIVATE);
        directory = new File(trustedFiles(context), "diagnostics");
        File marker = new File(directory, "enabled");
        boolean restored = available(context) && preferences.getBoolean("enabled", false) && consentMarker(directory, marker);
        session = new DiagnosticsSession(restored, preferences.getString("endpoint", DEFAULT_ENDPOINT), preferences.getString("token", ""));
        worker.scheduleWithFixedDelay(this::requestTick, 0, 30, TimeUnit.SECONDS);
    }
    boolean enabled() { return available(context) && session.current().enabled; }
    String endpoint() { return session.current().endpoint; }
    String token() { return session.current().token; }
    String status() { return enabled() ? status : "Off"; }

    synchronized void configure(boolean enabled, String endpoint, String token) throws Exception {
        if (!available(context)) throw new IllegalStateException("Only test builds support LAN diagnostics");
        pausedGeneration = -1;
        session.configure(enabled, endpoint, token, (on, url, secret) -> {
            if (Files.isSymbolicLink(directory.toPath()) || (!directory.isDirectory() && !directory.mkdirs()))
                throw new java.io.IOException("Diagnostics storage unavailable");
            File marker = new File(directory, "enabled");
            // Fail closed before validation and preference IO, including failed opt-out commits.
            Files.deleteIfExists(marker.toPath());
            if (url.length() > 256 || secret.length() > 128) throw new java.io.IOException("Pairing settings too long");
            if (on) {
                DiagnosticsTransport.validateEndpoint(url);
                if (!secret.matches("[A-Za-z0-9_-]{32,128}")) throw new java.io.IOException("Invalid pairing token");
            }
            if (!preferences.edit().putBoolean("enabled", on).putString("endpoint", url).putString("token", secret).commit())
                throw new java.io.IOException("Cannot save diagnostics settings");
            if (on) Files.write(marker.toPath(), new byte[]{1}, java.nio.file.StandardOpenOption.CREATE_NEW, java.nio.file.StandardOpenOption.WRITE);
        });
        status = enabled ? "Waiting for PC" : "Off";
        requestTick();
    }
    private void requestTick() {
        if (queued.compareAndSet(false, true)) worker.execute(() -> {
            try { tick(); } finally { queued.set(false); }
        });
    }
    /** Explicit launcher action; never changes pairing, consent or pending evidence. */
    void retryNow() {
        pausedGeneration = -1;
        retryRequested.set(true);
        worker.execute(this::requestTick);
    }
    interface ExportDestination { java.io.OutputStream open() throws java.io.IOException; }
    interface ExportResult { void completed(boolean success); }
    /** Explicit local export works with uploads OFF and without queue admission. */
    void export(ExportDestination destination, ExportResult result) {
        if (!exporting.compareAndSet(false, true)) { result.completed(false); return; }
        worker.execute(() -> {
            File candidate = new File(directory, "export-scratch.zip");
            boolean success = false;
            try {
                if (!available(context)) throw new java.io.IOException("Test support unavailable");
                prepareScratch(candidate);
                DiagnosticsSession.Snapshot snapshot = session.current();
                DiagnosticsArchive.Snapshot archive = DiagnosticsArchive.collect(new File(trustedFiles(context), "home/logs"), candidate,
                    sourceIdentity(), runtimeSummary(), snapshot.token);
                try (java.io.OutputStream output = destination.open()) { DiagnosticsFresh.copy(archive, candidate, output); }
                success = true;
            } catch (Exception ignored) { /* Never render a provider path or secret-bearing error. */ }
            finally { deleteScratch(candidate); exporting.set(false); }
            result.completed(success);
        });
    }
    private void ensureDirectory() throws java.io.IOException {
        if (Files.isSymbolicLink(directory.toPath()) || (!directory.isDirectory() && !directory.mkdirs()))
            throw new java.io.IOException("Storage unavailable");
    }
    private void prepareScratch(File candidate) throws java.io.IOException {
        ensureDirectory();
        if (Files.isSymbolicLink(candidate.toPath())) throw new java.io.IOException("Scratch unavailable");
        // Only these two generated scratch names are recycled; queue/log files are untouched.
        Files.deleteIfExists(candidate.toPath());
    }
    private void deleteScratch(File candidate) {
        try { if (!Files.isSymbolicLink(directory.toPath())) Files.deleteIfExists(candidate.toPath()); }
        catch (Exception ignored) { }
    }
    private boolean active(DiagnosticsSession.Snapshot snapshot) {
        return session.current() == snapshot && !snapshot.cancellation.isCancelled();
    }
    private void tick() {
        DiagnosticsSession.Snapshot snapshot = session.current();
        if (!available(context) || !snapshot.enabled
                || (pausedGeneration == snapshot.generation && !retryRequested.get())) return;
        boolean force = retryRequested.getAndSet(false);
        if (force) pausedGeneration = -1;
        File candidate = new File(directory, "live-scratch.zip");
        try {
            prepareScratch(candidate);
            String pair = DiagnosticsQueue.pairingKey(snapshot.endpoint, snapshot.token);
            DiagnosticsQueue queue = null;
            try { queue = new DiagnosticsQueue(new File(directory, "queue"), pair); }
            catch (Exception ignored) { /* Fresh collection/export does not depend on queue repair. */ }
            DiagnosticsFresh.Result fresh = null;
            try {
                fresh = DiagnosticsFresh.collect(new File(trustedFiles(context), "home/logs"), candidate,
                    sourceIdentity(), runtimeSummary(), snapshot.token, queue);
            } catch (Exception ignored) {
                // Queue retries must still work if current collection fails.
                status = "Fresh collection unavailable; original logs kept";
            }
            if (!active(snapshot)) return;
            boolean sentFresh = false;
            if (fresh != null && !fresh.retained
                    && !(pair.equals(directAckPair) && fresh.archive.snapshotId.equals(directAckSnapshot))) {
                // This is NEW evidence filtered with the CURRENT consent/token, never
                // an old-pair queued archive. One <=16 MiB scratch upload per tick.
                status = "Sending fresh test logs; pending reports kept";
                try {
                    DiagnosticsFresh.Result current = fresh;
                    sentFresh = DiagnosticsFresh.send(current, session, snapshot, cancellation ->
                        DiagnosticsTransport.upload(new URL(snapshot.endpoint), snapshot.token, candidate,
                            current.archive.archiveSha256, cancellation));
                    if (!active(snapshot)) return;
                    if (sentFresh) { directAckPair = pair; directAckSnapshot = fresh.archive.snapshotId; }
                } catch (Exception error) {
                    if (!active(snapshot)) return;
                    if (pairingRejected(error)) {
                        pausedGeneration = snapshot.generation;
                        status = "Pairing rejected; local export still available";
                        return;
                    }
                    status = "Fresh upload failed; original and pending logs kept";
                }
            }
            if (!active(snapshot)) return;
            if (queue == null) { status = sentFresh ? "Fresh logs received; pending storage unavailable"
                : "Pending storage unavailable; fresh local export available"; return; }
            DiagnosticsQueue.Entry entry = queue.next(force ? Long.MAX_VALUE : System.currentTimeMillis());
            if (entry == null) {
                if (fresh == null) status = "Fresh collection failed; originals kept; retry or export locally";
                else if (sentFresh) status = "Fresh logs received; older pending reports kept";
                else if (fresh != null && !fresh.retained) status = "Pending reports retained; fresh export available";
                else status = queue.totalCount() > queue.count() ? "Older pairing retained; local export available"
                    : queue.count() > 0 ? "Waiting to retry; local export available" : "Up to date";
                return;
            }
            status = "Sending queued test logs";
            try {
                DiagnosticsTransport.upload(new URL(snapshot.endpoint), snapshot.token, entry.file, entry.sha256, snapshot.cancellation);
                if (active(snapshot)) { queue.acknowledge(entry); status = "Received by PC"; }
            } catch (Exception error) {
                if (active(snapshot)) {
                    if (pairingRejected(error)) {
                        pausedGeneration = snapshot.generation;
                        status = "Pairing rejected; local export still available";
                    } else {
                        // A single rejected bundle must not pause all fresh collection.
                        queue.failed(entry, System.currentTimeMillis());
                        status = sentFresh ? "Fresh logs received; older report waiting to retry"
                            : "PC unavailable or report rejected; logs kept for retry/export";
                    }
                }
            }
        } catch (Exception ignored) {
            if (active(snapshot)) status = "Pending logs retained; local export available";
        } finally { deleteScratch(candidate); }
    }
    private static boolean pairingRejected(Exception error) {
        if (!(error instanceof DiagnosticsTransport.UploadException)) return false;
        int status = ((DiagnosticsTransport.UploadException)error).statusCode;
        return status == 401 || status == 403;
    }
    private String sourceIdentity() throws Exception {
        String source = asset("SOURCE.txt"), nativeIdentity = "";
        // Generic builds without an attested native revision omit it rather than guess.
        // The Java-only support APK's packaging gate requires this separate asset.
        try { nativeIdentity = "\nnative_source_revision=" + revision(asset("NATIVE-SOURCE.txt")); }
        catch (java.io.IOException absent) { }
        return "source_revision=" + revision(source) + nativeIdentity
            + "\npackage_version=" + context.getPackageManager().getPackageInfo(context.getPackageName(), 0).versionName
            + "\nandroid_api=" + Build.VERSION.SDK_INT + "\n";
    }
    private String asset(String name) throws Exception {
        try (InputStream stream = context.getAssets().open(name)) {
            java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[1024]; int count;
            while ((count = stream.read(buffer)) != -1) {
                if (bytes.size() + count > 4096) throw new java.io.IOException("Build identity too large");
                bytes.write(buffer, 0, count);
            }
            return new String(bytes.toByteArray(), StandardCharsets.UTF_8);
        }
    }
    private static String revision(String text) throws java.io.IOException {
        Matcher matcher = Pattern.compile("/tree/([0-9a-f]{40})(?:\\s|$)").matcher(text);
        if (!matcher.find()) throw new java.io.IOException("Build source identity missing");
        return matcher.group(1);
    }
    private String runtimeSummary() throws Exception {
        StringBuilder text = new StringBuilder();
        if (Build.VERSION.SDK_INT >= 30) {
            ActivityManager manager = (ActivityManager) context.getSystemService(Context.ACTIVITY_SERVICE);
            for (ApplicationExitInfo exit : manager.getHistoricalProcessExitReasons(context.getPackageName(), 0, 6)) {
                if (exit.getRealUid() == context.getApplicationInfo().uid
                        && (context.getPackageName() + ":game").equals(exit.getProcessName())) {
                    text.append("exit_pid=").append(exit.getPid()).append("\nexit_timestamp_ms=").append(exit.getTimestamp())
                        .append("\nexit_reason=").append(exit.getReason()).append("\nexit_status=").append(exit.getStatus()).append('\n');
                }
            }
        }
        File journal = new File(directory, "restart.log");
        if (journal.isFile() && !Files.isSymbolicLink(journal.toPath()) && journal.length() <= 8192)
            text.append(new String(Files.readAllBytes(journal.toPath()), StandardCharsets.UTF_8));
        return text.toString();
    }
    /** Fixed phase names and app-owned PIDs only; no Intent arguments, save paths or tokens. */
    static void record(Context context, String phase, int relatedPid) {
        if (!available(context) || !phase.matches("[A-Z_]{1,40}")) return;
        File directory = new File(trustedFiles(context), "diagnostics");
        if (Files.isSymbolicLink(directory.toPath())) return;
        if (!new File(directory, "enabled").isFile()) return;
        File journal = new File(directory, "restart.log");
        java.util.Set<java.nio.file.OpenOption> options = new java.util.HashSet<>(java.util.Arrays.asList(
            java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.READ, java.nio.file.StandardOpenOption.WRITE,
            java.nio.file.LinkOption.NOFOLLOW_LINKS));
        try (FileChannel file = FileChannel.open(journal.toPath(), options); FileLock lock = file.tryLock()) {
            if (lock == null || Files.isSymbolicLink(directory.toPath())) return;
            if (file.size() > 4096) {
                int count = (int)Math.min(2048, file.size());
                ByteBuffer tail = ByteBuffer.allocate(count); file.position(file.size() - count);
                while (tail.hasRemaining() && file.read(tail) > 0) { }
                String text = new String(tail.array(), 0, tail.position(), StandardCharsets.US_ASCII);
                int newline = text.indexOf('\n'); text = newline < 0 ? "" : text.substring(newline + 1);
                file.truncate(0); file.position(0); write(file, text);
            }
            file.position(file.size());
            write(file, "event=" + phase + " pid=" + android.os.Process.myPid() + " time_ms=" + System.currentTimeMillis()
                + (relatedPid >= 0 ? " related_pid=" + relatedPid : "") + "\n");
        } catch (Exception ignored) { /* Evidence must never prevent gameplay or restart. */ }
    }
    private static void write(FileChannel file, String text) throws java.io.IOException {
        ByteBuffer bytes = ByteBuffer.wrap(text.getBytes(StandardCharsets.US_ASCII));
        while (bytes.hasRemaining()) file.write(bytes);
    }
    private static File trustedFiles(Context context) {
        try { return context.getFilesDir().getCanonicalFile(); }
        catch (java.io.IOException error) { return context.getFilesDir(); }
    }
    private static boolean consentMarker(File directory, File marker) {
        try {
            return !Files.isSymbolicLink(directory.toPath()) && !Files.isSymbolicLink(marker.toPath())
                && marker.isFile() && marker.length() == 1 && Files.readAllBytes(marker.toPath())[0] == 1;
        } catch (Exception error) { return false; }
    }
}
