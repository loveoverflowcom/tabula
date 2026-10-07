package com.loveoverflow.tabula.mobile.host

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * The seam between the Compose shell and the surface that runs a game.
 *
 * The shell decides when a game is shown and reacts to [GameHostEvent]; the host owns the
 * surface and its lifecycle. A host must create its runtime when it enters the composition and
 * only then: a recomposition, a new [onEvent] lambda or a changed [modifier] never restarts it.
 * Neither side carries rules, canonical state or per-frame drawing commands across this boundary.
 * Native adapters separate runtime, surface and match lifetime (ADR-0043): surface recreation cannot
 * start a new match; retired callbacks are fenced before disposal and render-thread teardown completes
 * before reopen. The placeholder never reports Ready; native implementation/evidence is still owed.
 */
interface GameHost {
    @Composable
    fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort)
}
