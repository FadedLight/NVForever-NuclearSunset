package org.nvforever.android;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;

/** Imports a copy through the directory picker; never writes to the source tree. */
public final class LauncherActivity extends Activity {
    private MobileLauncher launcher;
    private boolean busy;
    private static final int PICK_GAME = 1;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (android.os.Build.VERSION.SDK_INT >= 30) getWindow().setDecorFitsSystemWindows(false);
        getWindow().setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        try {
            launcher = new MobileLauncher(this);
        } catch (IOException | org.json.JSONException | LinkageError e) {
            new android.app.AlertDialog.Builder(this).setTitle("NVForever launcher")
                .setMessage("Could not open the launcher: " + e.getMessage())
                .setPositiveButton("Close", (dialog, which) -> finish()).setCancelable(false).show();
        }
    }
    @Override public void onResume() {
        super.onResume();
        if (launcher == null) return;
        String error = getIntent().getStringExtra("error");
        getIntent().removeExtra("error");
        launcher.refresh(busy, runningGame(), error);
        launcher.resume();
    }
    @Override protected void onPause() {
        if (launcher != null) launcher.pause();
        super.onPause();
    }
    @Override protected void onNewIntent(Intent intent) { super.onNewIntent(intent); setIntent(intent); }
    void pickGame() {
        if (busy || runningGame()) return;
        startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION), PICK_GAME);
    }
    boolean runningGame() {
        android.app.ActivityManager manager = (android.app.ActivityManager) getSystemService(ACTIVITY_SERVICE);
        var processes = manager.getRunningAppProcesses();
        if (processes != null) for (var p : processes)
            if (p.uid == android.os.Process.myUid() && p.processName.equals(getPackageName() + ":game")) return true;
        return false;
    }
    private void working(boolean value, String message) {
        busy = value;
        if (!isDestroyed()) launcher.refresh(value, runningGame(), message);
    }
    @Override public void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (launcher == null) return;
        if (request != PICK_GAME || result != RESULT_OK || data == null || data.getData() == null) return;
        Uri tree = data.getData();
        if (busy || runningGame()) { launcher.message("Close the game before importing another install."); return; }
        working(true, "Copying game files. Keep NVForever open until this finishes…");
        new Thread(() -> {
            File imported = null;
            boolean selected = false;
            try {
                File external = getExternalFilesDir(null);
                if (external == null) throw new IOException("App storage is unavailable");
                imported = new File(external, "game-" + java.util.UUID.randomUUID());
                if (!imported.mkdirs()) throw new IOException("Cannot create import directory");
                if (!new File(imported, ".nvf-import").createNewFile()) throw new IOException("Cannot mark import directory");
                String root = DocumentsContract.getTreeDocumentId(tree);
                String child = findChild(tree, root, "Data");
                String dataRoot = child == null ? root : child;
                if (findChild(tree, dataRoot, "FalloutNV.esm") == null) throw new IOException("Select the game folder or its Data folder (FalloutNV.esm is missing)");
                for (String required : InstallConfig.REQUIRED) {
                    if (findChild(tree, dataRoot, required) == null) throw new IOException("The selected game is incomplete: missing " + required);
                }
                copyDirectory(tree, dataRoot, new File(imported, "Data"), new HashSet<>(), 0);
                launcher.validateImport(imported.getAbsolutePath());
                if (child != null) {
                    String ini = findChild(tree, root, "Fallout_default.ini");
                    if (ini != null) copyFile(tree, ini, new File(imported, "Fallout_default.ini"));
                }
                File home = new File(getFilesDir(), "home");
                if (!home.isDirectory() && !home.mkdirs()) throw new IOException("Cannot create settings directory");
                android.util.AtomicFile config = new android.util.AtomicFile(new File(home, "NVForever.ini"));
                FileOutputStream out = null;
                try {
                    String old = "";
                    if (config.getBaseFile().isFile()) try (InputStream in = config.openRead()) {
                        java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
                        byte[] buffer = new byte[8192]; int n;
                        while ((n = in.read(buffer)) != -1) bytes.write(buffer, 0, n);
                        old = bytes.toString("UTF-8");
                    }
                    String updated = InstallConfig.updateGamePath(old, imported.getAbsolutePath());
                    out = config.startWrite(); out.write(updated.getBytes(StandardCharsets.UTF_8)); config.finishWrite(out);
                } catch (Exception e) { if (out != null) config.failWrite(out); throw e; }
                selected = true;
                var preferences = getSharedPreferences("imports", MODE_PRIVATE);
                String previous = preferences.getString("active", "");
                if (preferences.edit().putString("active", imported.getAbsolutePath()).commit() && !previous.isEmpty()) removeImport(new File(previous));
                runOnUiThread(() -> working(false, "Game files imported. Ready to start."));
            } catch (Exception e) { runOnUiThread(() -> working(false, "Import failed: " + e.getMessage() + ". The previous import is still selected.")); }
            finally { if (!selected && imported != null) removeImport(imported); }
        }, "nvf-import").start();
    }
    private void removeImport(File root) {
        try {
            File external = getExternalFilesDir(null);
            if (external == null || !InstallConfig.ownedImport(external, root)) return;
            String active = getSharedPreferences("imports", MODE_PRIVATE).getString("active", "");
            if (root.getCanonicalPath().equals(active)) return;
            deleteOwnedTree(root);
        } catch (IOException ignored) { /* An incomplete copy may be removed on app uninstall. */ }
    }
    private void deleteOwnedTree(File file) throws IOException {
        if (!java.nio.file.Files.isSymbolicLink(file.toPath()) && file.isDirectory()) {
            File[] children = file.listFiles();
            if (children == null) throw new IOException("Cannot list import directory");
            for (File child : children) deleteOwnedTree(child);
        }
        if (!file.delete()) throw new IOException("Cannot remove old import file");
    }
    private String findChild(Uri tree, String parent, String name) throws IOException {
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent);
        try (Cursor c = getContentResolver().query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (c == null) throw new IOException("Cannot read the selected folder");
            while (c.moveToNext()) if (name.equalsIgnoreCase(c.getString(1))) return c.getString(0);
        }
        return null;
    }
    private void copyDirectory(Uri tree, String id, File target, Set<String> visited, int depth) throws IOException {
        if (depth > 128 || !visited.add(id)) throw new IOException("Recursive document tree refused");
        if (!target.mkdirs()) throw new IOException("Cannot create " + target.getName());
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, id);
        try (Cursor c = getContentResolver().query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE}, null, null, null)) {
            if (c == null) throw new IOException("Cannot read a game directory");
            while (c.moveToNext()) {
                String name = c.getString(1);
                InstallConfig.checkName(name);
                File child = new File(target, name);
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(c.getString(2))) copyDirectory(tree, c.getString(0), child, visited, depth + 1);
                else copyFile(tree, c.getString(0), child);
            }
        }
    }
    private void copyFile(Uri tree, String id, File target) throws IOException {
        try (InputStream in = getContentResolver().openInputStream(DocumentsContract.buildDocumentUriUsingTree(tree, id)); FileOutputStream out = new FileOutputStream(target)) {
            if (in == null) throw new IOException("Cannot open " + target.getName());
            byte[] buffer = new byte[1024 * 1024]; int n;
            while ((n = in.read(buffer)) != -1) out.write(buffer, 0, n);
        }
    }
    void prepareAndStart(boolean resume) {
        if (busy || runningGame()) return;
        final String args;
        try {
            args = launcher.startArgs(resume);
        } catch (IOException | org.json.JSONException e) {
            launcher.startRefused(e.getMessage());
            return;
        }
        working(true, "Preparing engine schemas…");
        new Thread(() -> {
            try {
                copyAsset("schemas", new File(getFilesDir(), "runtime/schemas"));
                copyAsset("lang", new File(getFilesDir(), "home/lang"));
                runOnUiThread(() -> {
                    if (isDestroyed() || isFinishing()) return;
                    busy = false;
                    launcher.message("");
                    startActivity(new Intent(this, GameActivity.class).putExtra("args", args));
                });
            } catch (Exception e) { runOnUiThread(() -> working(false, "Startup failed: " + e.getMessage())); }
        }, "nvf-setup").start();
    }
    private void copyAsset(String asset, File target) throws IOException {
        String[] children = getAssets().list(asset);
        if (children != null && children.length > 0) {
            if (!target.isDirectory() && !target.mkdirs()) throw new IOException("Cannot extract schemas");
            for (String name : children) copyAsset(asset + "/" + name, new File(target, name));
        } else try (InputStream in = getAssets().open(asset); FileOutputStream out = new FileOutputStream(target)) {
            byte[] buffer = new byte[65536]; int n;
            while ((n = in.read(buffer)) != -1) out.write(buffer, 0, n);
        }
    }
}
