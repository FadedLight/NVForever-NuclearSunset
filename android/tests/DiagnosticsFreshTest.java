package org.nvforever.android;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.zip.ZipFile;

/** Real synthetic files/ZIP/queue; no Android runtime or network. */
public final class DiagnosticsFreshTest {
    private static final String SOURCE = "source_revision=0123456789abcdef0123456789abcdef01234567\nandroid_api=36\n";
    private static final String TOKEN = "synthetic_pairing_token_01234567890123456789";
    private static final String ENDPOINT = "http://192.168.1.20:8765/api/v1/log-bundles";
    private static int checks;
    private static void check(boolean condition, String label) { checks++; if (!condition) throw new AssertionError(label); }
    private static String id(int n) { return String.format("%064x", n); }
    private static String hash(byte[] bytes) throws Exception {
        StringBuilder out = new StringBuilder();
        for (byte b : MessageDigest.getInstance("SHA-256").digest(bytes)) out.append(String.format("%02x", b & 255));
        return out.toString();
    }
    private static Map<String, String> files(Path directory) throws Exception {
        Map<String, String> values = new TreeMap<>();
        try (var paths = Files.list(directory)) {
            for (Path path : paths.toArray(Path[]::new)) if (!path.getFileName().toString().equals(".lock"))
                values.put(path.getFileName().toString(), hash(Files.readAllBytes(path)));
        }
        return values;
    }
    private static Path logs(Path root) throws Exception {
        Path logs = Files.createDirectories(root.resolve("home/logs"));
        Files.writeString(logs.resolve("nvforever-20261114-120000-42.log"),
            "render: pipeline ready\nrender: player=PRIVATE_PLAYER token=" + TOKEN
            + "\nrender: model not loaded /private/person/secret.nif\n");
        Files.writeString(logs.resolve("unrelated.txt"), "PRIVATE_UNRELATED");
        return logs;
    }
    private static void filtered(File archive) throws Exception {
        StringBuilder text = new StringBuilder();
        try (ZipFile zip = new ZipFile(archive)) {
            var entries = zip.entries();
            while (entries.hasMoreElements()) {
                var entry = entries.nextElement();
                check(entry.getName().equals("manifest.json") || entry.getName().matches("logs/entry-[0-9]+\\.txt"), "anonymous allowed ZIP name");
                text.append(new String(zip.getInputStream(entry).readAllBytes(), StandardCharsets.UTF_8));
            }
        }
        check(text.toString().contains("pipeline ready"), "actual fresh technical evidence retained");
        check(!text.toString().contains("PRIVATE_") && !text.toString().contains(TOKEN)
            && !text.toString().contains("/private/person"), "private identity/token/path/out-of-scope omitted");
        check(archive.length() <= DiagnosticsArchive.MAX_ARCHIVE_BYTES, "bounded output");
    }
    private static void fullQueue(Path root, boolean oldPairs) throws Exception {
        Path logs = logs(root); Path queueDir = root.resolve("queue");
        File original = root.resolve("original.zip").toFile();
        var originalArchive = DiagnosticsArchive.collect(logs.toFile(), original, SOURCE, "", TOKEN);
        String currentPair = DiagnosticsQueue.pairingKey(ENDPOINT, TOKEN);
        for (int i = 0; i < 4; i++) {
            String pair = oldPairs ? DiagnosticsQueue.pairingKey(ENDPOINT, TOKEN + i) : currentPair;
            var q = new DiagnosticsQueue(queueDir.toFile(), pair);
            check(q.enqueue(original, id(i + 1), originalArchive.archiveSha256), "populate real retained queue");
        }
        var queue = new DiagnosticsQueue(queueDir.toFile(), currentPair);
        check(!queue.canEnqueue(), "full queue reproduces old collection gate");
        Map<String, String> before = files(queueDir);
        File candidate = root.resolve("fresh.zip").toFile();
        var fresh = DiagnosticsFresh.collect(logs.toFile(), candidate, SOURCE, "exit_status=11\n", TOKEN, queue);
        check(!fresh.retained && candidate.isFile(), "fresh evidence exists despite full queue");
        filtered(candidate);
        check(files(queueDir).equals(before), "all four old payload AND metadata bytes untouched");
        check(queue.totalCount() == 4 && queue.count() == (oldPairs ? 0 : 4), "pair isolation unchanged");
        var session = new DiagnosticsSession(true, ENDPOINT, TOKEN);
        AtomicInteger sends = new AtomicInteger();
        check(DiagnosticsFresh.send(fresh, session, session.current(), cancel -> {
            check(!cancel.isCancelled(), "current cancellation forwarded");
            filtered(candidate); sends.incrementAndGet();
        }) && sends.get() == 1, "only NEW current-consent scratch eligible for fallback");
        check(files(queueDir).equals(before), "fresh send cannot ACK/reassign/delete old pairing");
        check(original.isFile(), "original generated candidate retained");
    }
    private static void revokeAndExport(Path root) throws Exception {
        Path logs = logs(root); File candidate = root.resolve("fresh.zip").toFile();
        var session = new DiagnosticsSession(true, ENDPOINT, TOKEN);
        var snapshot = session.current(); AtomicInteger sends = new AtomicInteger();
        // An injected enqueue commit cancels consent WHILE real filtered collection
        // and admission are in progress, then refuses retention to exercise fallback.
        var queue = new DiagnosticsQueue(root.resolve("queue").toFile(), DiagnosticsQueue.pairingKey(ENDPOINT, TOKEN), () -> {
            session.configure(false, ENDPOINT, TOKEN, (on, url, secret) -> { });
            throw new java.io.IOException("Synthetic interrupted admission");
        });
        var fresh = DiagnosticsFresh.collect(logs.toFile(), candidate, SOURCE, "", TOKEN, queue);
        check(!fresh.retained && snapshot.cancellation.isCancelled(), "revocation during collection propagated");
        check(!DiagnosticsFresh.send(fresh, session, snapshot, cancel -> sends.incrementAndGet())
            && sends.get() == 0, "revoked generation cannot start fallback upload");
        session.configure(true, ENDPOINT, TOKEN + "new", (on, url, secret) -> { });
        check(!DiagnosticsFresh.send(fresh, session, snapshot, cancel -> sends.incrementAndGet())
            && sends.get() == 0, "new pairing cannot inherit old in-flight snapshot");
        var current = session.current();
        check(!DiagnosticsFresh.send(fresh, session, current, cancel -> {
            session.configure(false, ENDPOINT, TOKEN, (on, url, secret) -> { });
            check(cancel.isCancelled(), "OFF cancels in-flight context");
        }), "ACK after OFF not accepted");
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        DiagnosticsFresh.copy(fresh.archive, candidate, output);
        check(java.util.Arrays.equals(output.toByteArray(), Files.readAllBytes(candidate.toPath())),
            "explicit local export with uploads OFF is exactly same filtered ZIP");
        filtered(candidate);
        check(queue.totalCount() == 0, "failed admission exposes no partial pending evidence");
    }
    private static void availableAndUnavailableQueue(Path root) throws Exception {
        Path logs = logs(root); var queue = new DiagnosticsQueue(root.resolve("queue").toFile(),
            DiagnosticsQueue.pairingKey(ENDPOINT, TOKEN));
        var queued = DiagnosticsFresh.collect(logs.toFile(), root.resolve("queued.zip").toFile(), SOURCE, "", TOKEN, queue);
        check(queued.retained && queue.count() == 1, "available queue retains fresh immutable archive");
        var session = new DiagnosticsSession(true, ENDPOINT, TOKEN);
        check(!DiagnosticsFresh.send(queued, session, session.current(), cancel -> { throw new AssertionError("duplicate send"); }),
            "durable queue is authoritative when admitted");
        Files.writeString(root.resolve("queue/unexpected.txt"), "preserve synthetic evidence");
        Map<String, String> before = files(root.resolve("queue"));
        var unavailable = DiagnosticsFresh.collect(logs.toFile(), root.resolve("unavailable.zip").toFile(), SOURCE, "", TOKEN, queue);
        check(!unavailable.retained && files(root.resolve("queue")).equals(before), "unexpected queue files are not removed/repaired");
        filtered(root.resolve("unavailable.zip").toFile());
        var exported = DiagnosticsFresh.collect(logs.toFile(), root.resolve("no-queue.zip").toFile(), SOURCE, "", TOKEN, null);
        check(!exported.retained, "local collection needs no queue or upload consent");
        filtered(root.resolve("no-queue.zip").toFile());
    }
    public static void main(String[] args) throws Exception {
        Path root = Files.createTempDirectory("nvf-fresh-diagnostics-");
        try {
            fullQueue(Files.createDirectory(root.resolve("full")), false);
            fullQueue(Files.createDirectory(root.resolve("old-pairs")), true);
            revokeAndExport(Files.createDirectory(root.resolve("revoke")));
            availableAndUnavailableQueue(Files.createDirectory(root.resolve("available")));
            System.out.println("DiagnosticsFreshTest: PASS (" + checks + " checks; no network)");
        } finally {
            try (var paths = Files.walk(root)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toArray(Path[]::new)) Files.deleteIfExists(path);
            }
        }
    }
}
