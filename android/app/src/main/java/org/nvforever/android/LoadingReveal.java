package org.nvforever.android;

/** Shared production completion protocol. Host owns the actual Activity effects. */
final class LoadingReveal {
    interface Host {
        LoadingState state();
        LoadingUi.CoverLease cover();
        boolean active();
        void completed();
        void failed();
    }
    static void schedule(Host host) {
        final String ticket = host.state().ticket();
        if (host.cover() == null) return;
        host.cover().nextFrame(() -> {
            if (!host.active() || !host.state().reveal(ticket)) return;
            host.cover().hide();
            host.completed();
        });
    }
    private LoadingReveal() { }
}
