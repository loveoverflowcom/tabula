package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameBackPort
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.voice.VoiceController
import com.loveoverflow.tabula.mobile.voice.VoiceControls

/** Home uses Design 01's display hierarchy and warm hero; only packaged local play is actionable. */
@Composable
fun HomeScreen(games: List<BundledGame>, strings: ShellStrings, onOpen: (BundledGame) -> Unit, onBrowse: () -> Unit) {
    val colors = LocalTabulaColors.current
    ShellPage(strings[ShellCopy.HomeHeading], Modifier.testTag("shell-home"), displayTitle = true) {
        ShellSurface(hero = true) {
            TabulaText(strings[ShellCopy.HomeIntro], TabulaType.displaySm, Modifier.semantics { heading() }, colors.shellOnHero)
            if (games.isEmpty()) {
                TabulaText(strings[ShellCopy.NoGamesTitle], TabulaType.titleMd, Modifier.semantics { heading() }, colors.shellOnHero)
                TabulaText(strings[ShellCopy.NoGames], TabulaType.bodyMd, color = colors.shellOnHero)
            } else {
                TabulaText(strings[ShellCopy.PackagedGames], TabulaType.titleLg, Modifier.semantics { heading() }, colors.shellOnHero)
                TabulaText(strings[ShellCopy.LocalOnly], TabulaType.bodyMd, color = colors.shellOnHero)
                for ((index, game) in games.withIndex()) {
                    ShellButton(
                        strings.play(game.displayName(strings.languageTag)),
                        filled = index == 0,
                        onClick = { onOpen(game) },
                        modifier = Modifier.fillMaxWidth().testTag("shell-play-${game.id}"),
                    )
                }
            }
        }
        CatalogUnavailable(strings) {
            ShellActionButton(
                strings[ShellCopy.BrowseGames], ShellAction.Text, onBrowse,
                Modifier.fillMaxWidth().testTag("shell-browse-games"),
            )
        }
    }
}

/** Packaged entries can open detail; the full registry catalog remains a clearly labeled gate. */
@Composable
fun GamesScreen(games: List<BundledGame>, strings: ShellStrings, onDetail: (BundledGame) -> Unit) {
    ShellPage(strings[ShellCopy.Games], Modifier.testTag("shell-games"), displayTitle = true) {
        if (games.isEmpty()) {
            ShellStatePanel(strings[ShellCopy.NoGamesTitle], strings[ShellCopy.NoGames])
        } else {
            TabulaText(strings[ShellCopy.PackagedGames], TabulaType.titleLg, Modifier.semantics { heading() })
            for (game in games) {
                ShellSurface {
                    val name = game.displayName(strings.languageTag)
                    TabulaText(name, TabulaType.titleLg, Modifier.semantics { heading() })
                    TabulaText(strings[ShellCopy.LocalOnly], TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
                    ShellActionButton(
                        strings.details(name), ShellAction.Tonal, { onDetail(game) },
                        Modifier.fillMaxWidth().testTag("shell-details-${game.id}"),
                    )
                }
            }
        }
        CatalogUnavailable(strings)
    }
}

/** Local detail is limited to manifest display data; it invents no rules, seats, art or capabilities. */
@Composable
fun DetailScreen(game: BundledGame?, strings: ShellStrings, onSetup: () -> Unit) {
    ShellPage(game?.displayName(strings.languageTag) ?: strings[ShellCopy.Details], Modifier.testTag("shell-detail")) {
        if (game == null) {
            ShellStatePanel(strings[ShellCopy.DetailUnavailableTitle], strings[ShellCopy.DetailUnavailable])
        } else {
            ShellSurface {
                TabulaText(strings[ShellCopy.LocalMode], TabulaType.titleLg, Modifier.semantics { heading() })
                TabulaText(strings[ShellCopy.LocalOnly], TabulaType.bodyMd)
                ShellActionButton(strings[ShellCopy.Setup], ShellAction.Filled, onSetup,
                    Modifier.fillMaxWidth().testTag("shell-setup-action"))
            }
            OnlineUnavailable(strings)
        }
    }
}

/** Setup preserves the packaged launch query; this scaffold adds no game configuration or network action. */
@Composable
fun SetupScreen(game: BundledGame?, strings: ShellStrings, onPlay: () -> Unit) {
    ShellPage(game?.let { strings.setup(it.displayName(strings.languageTag)) } ?: strings[ShellCopy.SetupTitle], Modifier.testTag("shell-setup")) {
        if (game == null) {
            ShellStatePanel(strings[ShellCopy.DetailUnavailableTitle], strings[ShellCopy.DetailUnavailable])
        } else {
            ShellSurface {
                TabulaText(strings[ShellCopy.LocalMode], TabulaType.titleLg, Modifier.semantics { heading() })
                TabulaText(strings[ShellCopy.SetupIntro], TabulaType.bodyMd)
                ShellActionButton(strings[ShellCopy.StartLocalGame], ShellAction.Filled, onPlay,
                    Modifier.fillMaxWidth().testTag("shell-start-local"))
            }
            OnlineUnavailable(strings)
        }
    }
}

/** Anonymous account scaffold uses the shared neutral silhouette until a verified profile adapter exists. */
@Composable
fun AccountScreen(strings: ShellStrings) {
    ShellPage(strings[ShellCopy.AccountTitle], Modifier.testTag("shell-account")) {
        ShellSurface {
            Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp), verticalAlignment = Alignment.CenterVertically) {
                ShellAnonymousAvatar(strings)
                TabulaText(strings[ShellCopy.Anonymous], TabulaType.titleLg, Modifier.weight(1f))
            }
        }
        ShellStatePanel(
            strings[ShellCopy.AccountUnavailableTitle], strings[ShellCopy.AccountUnavailable],
            Modifier.testTag("shell-account-unavailable"),
        )
        TabulaText(strings[ShellCopy.PreferencesUnavailable], TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
    }
}

@Composable
private fun CatalogUnavailable(strings: ShellStrings, action: (@Composable () -> Unit)? = null) = ShellStatePanel(
    strings[ShellCopy.CatalogUnavailableTitle], strings[ShellCopy.CatalogUnavailable],
    Modifier.testTag("shell-catalog-unavailable"), action = action,
)

@Composable
private fun OnlineUnavailable(strings: ShellStrings) = ShellStatePanel(
    strings[ShellCopy.OnlineUnavailable], strings[ShellCopy.HomeStatus], Modifier.testTag("shell-online-unavailable"),
)

/**
 * Hosts the platform [GameHost] under a toolbar; the shell owns navigation, the host owns the surface.
 * Back goes to the host first. Unexplained host failure releases that surface; retry mounts one
 * fresh host and starts a new game. Recomposition and resize never change runtime identity.
 */
// This is the shared Back API available on both targets without a new navigation dependency.
@OptIn(ExperimentalComposeUiApi::class)
@Suppress("DEPRECATION")
@Composable
fun GameScreen(
    title: String,
    launch: GameLaunch,
    host: GameHost,
    onLeave: () -> Unit,
    voice: VoiceController? = null,
    vietnamese: Boolean = false,
    strings: ShellStrings = ShellStrings.forLanguage(if (vietnamese) "vi" else "en"),
) {
    val back = remember { GameBackPort() }
    var failure by remember { mutableStateOf<String?>(null) }
    val leave = { if (!back.requestBack()) onLeave() }
    BackHandler(enabled = true, onBack = leave)
    ShellPage(title, Modifier.background(LocalTabulaColors.current.shellCanvas).safeDrawingPadding(), scrollable = false) {
        Row { ShellButton(strings[ShellCopy.Back], filled = false, onClick = leave, modifier = Modifier.testTag("shell-back")) }
        if (voice != null) VoiceControls(voice, strings.vietnamese)
        val reason = failure
        if (reason != null) {
            FailurePanel(reason, strings, onRetry = { failure = null }, modifier = Modifier.fillMaxWidth().weight(1f))
        } else {
            // Exclusive composition branches discard the failed runtime; retry mounts a fresh one.
            host.Content(
                launch = launch,
                onEvent = { event ->
                    when {
                        event == GameHostEvent.Exited -> onLeave()
                        event is GameHostEvent.Failed && !event.shownByGame -> failure = event.reason
                    }
                },
                modifier = Modifier.fillMaxWidth().weight(1f),
                back = back,
            )
        }
    }
}

@Composable
private fun FailurePanel(reason: String, strings: ShellStrings, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        ShellStatePanel(
            strings[ShellCopy.FailureTitle], reason,
            Modifier.fillMaxWidth().testTag("shell-host-failure"), error = true,
        ) {
            TabulaText(strings[ShellCopy.FailureNote], TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
            ShellActionButton(strings[ShellCopy.Retry], ShellAction.Filled, onRetry, Modifier.testTag("shell-retry"))
        }
    }
}
