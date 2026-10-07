package com.loveoverflow.tabula.mobile.navigation

/**
 * One active account task may dismiss its local confirmation before the shell pops its route.
 * Toolbar and platform Back share this port; it contains no account/session or persisted state.
 */
class AccountTaskBackPort {
    private var handler: (() -> Boolean)? = null

    /** Registers the current task and returns an exact-owner cleanup, safe against late disposal. */
    fun register(onBack: () -> Boolean): () -> Unit {
        handler = onBack
        return { if (handler === onBack) handler = null }
    }

    /** True means a local task handled Back; false permits the shell to pop normally. */
    fun requestBack(): Boolean = handler?.invoke() ?: false
}
