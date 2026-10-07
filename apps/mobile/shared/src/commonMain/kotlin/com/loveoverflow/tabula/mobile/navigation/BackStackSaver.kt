package com.loveoverflow.tabula.mobile.navigation

import androidx.compose.runtime.saveable.listSaver

/**
 * Compose adapter for the independently testable public shell history. It saves only route paths,
 * never account identities/operations, credentials, private profile facts or an active local match.
 */
val BackStackSaver = listSaver<BackStack, String>(
    save = { it.saveRoutes() },
    restore = { BackStack.restoreRoutes(it) },
)
