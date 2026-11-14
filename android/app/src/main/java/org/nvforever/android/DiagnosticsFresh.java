package org.nvforever.android;

import java.io.File;
import java.io.IOException;

/** Fresh, filtered evidence is collectable even when durable pending reports are full.
 * The caller owns this single scratch file and its bounded transport/export lifetime.
 * Old queued reports and their pairing identities are never changed by this operation.
 */
public final class DiagnosticsFresh {
    private DiagnosticsFresh() { }
    public static final class Result {
        public final DiagnosticsArchive.Snapshot archive;
        public final boolean retained;
        private Result(DiagnosticsArchive.Snapshot archive, boolean retained) {
            this.archive = archive; this.retained = retained;
        }
    }
    public static Result collect(File logs, File candidate, String source, String runtime,
                                 String secret, DiagnosticsQueue queue) throws IOException {
        DiagnosticsArchive.Snapshot archive = DiagnosticsArchive.collect(logs, candidate, source, runtime, secret);
        // enqueue tests the actual size, rather than reserving the 16 MiB worst case.
        // A full/old-pair queue never suppresses the fresh filtered snapshot.
        boolean retained = false;
        if (queue != null) {
            try { retained = queue.containsSnapshot(archive.snapshotId)
                || queue.enqueue(candidate, archive.snapshotId, archive.archiveSha256); }
            catch (IOException unavailable) { /* Retain the fresh scratch; never repair or discard queue evidence. */ }
        }
        return new Result(archive, retained);
    }
    interface Upload { void send(DiagnosticsTransport.Cancellation cancellation) throws Exception; }
    /** Final same-generation consent check, including revocation during collection. */
    static boolean send(Result fresh, DiagnosticsSession session, DiagnosticsSession.Snapshot snapshot,
                        Upload upload) throws Exception {
        if (fresh.retained || session.current() != snapshot || !snapshot.enabled || snapshot.cancellation.isCancelled()) return false;
        upload.send(snapshot.cancellation);
        return session.current() == snapshot && !snapshot.cancellation.isCancelled();
    }
    /** Copies only an already filtered bounded archive; independent of queue/consent/network. */
    static void copy(DiagnosticsArchive.Snapshot archive, File candidate, java.io.OutputStream output) throws IOException {
        if (output == null || java.nio.file.Files.isSymbolicLink(candidate.toPath())
                || candidate.length() != archive.archiveBytes || archive.archiveBytes > DiagnosticsArchive.MAX_ARCHIVE_BYTES)
            throw new IOException("Export unavailable");
        try (java.io.InputStream input = java.nio.file.Files.newInputStream(candidate.toPath(),
                java.nio.file.StandardOpenOption.READ, java.nio.file.LinkOption.NOFOLLOW_LINKS)) {
            byte[] buffer = new byte[8192]; long count = 0; int n;
            while ((n = input.read(buffer)) != -1) {
                count += n;
                if (count > archive.archiveBytes) throw new IOException("Export limit");
                output.write(buffer, 0, n);
            }
            if (count != archive.archiveBytes) throw new IOException("Export changed");
            output.flush();
        }
    }
}
