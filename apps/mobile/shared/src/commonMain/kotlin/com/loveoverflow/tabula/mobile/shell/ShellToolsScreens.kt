package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.ShellToolsCopy
import com.loveoverflow.tabula.mobile.localization.tools

/** Native room availability only; this screen neither fabricates rooms nor admits a match. */
@Composable
fun RoomsScreen(strings: ShellStrings, onBrowse: () -> Unit) {
    val title = strings.tools(ShellToolsCopy.Rooms)
    ShellPage(title, Modifier.testTag("shell-rooms").semantics { paneTitle = title }) {
        TabulaText(strings.tools(ShellToolsCopy.RoomsIntro), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        ShellStatePanel(
            strings.tools(ShellToolsCopy.RoomsUnavailable), strings.tools(ShellToolsCopy.RoomsUnavailableBody),
            Modifier.testTag("shell-rooms-unavailable"),
            action = { ShellButton(strings[ShellCopy.BrowseGames], true, onBrowse, Modifier.testTag("rooms-open-library")) },
        )
    }
}

/** Native history availability only; canonical/replay/account records never enter saved routes. */
@Composable
fun HistoryScreen(strings: ShellStrings, onBrowse: () -> Unit) {
    val title = strings.tools(ShellToolsCopy.History)
    ShellPage(title, Modifier.testTag("shell-history").semantics { paneTitle = title }) {
        TabulaText(strings.tools(ShellToolsCopy.HistoryIntro), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        ShellStatePanel(
            strings.tools(ShellToolsCopy.HistoryUnavailable), strings.tools(ShellToolsCopy.HistoryUnavailableBody),
            Modifier.testTag("shell-history-unavailable"),
            action = { ShellButton(strings[ShellCopy.BrowseGames], true, onBrowse, Modifier.testTag("history-open-library")) },
        )
    }
}

/** Functional, account-independent local choices; default System preserves OS accessibility. */
@Composable
fun SettingsScreen(preferences: ShellPreferences, strings: ShellStrings, onChange: (ShellPreferences) -> Unit) {
    val title = strings.tools(ShellToolsCopy.Settings)
    ShellPage(title, Modifier.testTag("shell-settings").semantics { paneTitle = title }) {
        TabulaText(strings.tools(ShellToolsCopy.SettingsIntro), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        ShellSurface {
            PreferenceHeading(strings.tools(ShellToolsCopy.Appearance))
            Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
                for (choice in ShellAppearance.entries) ShellPreferenceChoice(
                    strings.tools(when (choice) {
                        ShellAppearance.System -> ShellToolsCopy.System
                        ShellAppearance.Light -> ShellToolsCopy.Light
                        ShellAppearance.Dark -> ShellToolsCopy.Dark
                    }), preferences.appearance == choice, { onChange(preferences.copy(appearance = choice)) },
                    Modifier.testTag("settings-appearance-${choice.savedCode}"),
                )
            }
        }
        ShellSurface {
            PreferenceHeading(strings.tools(ShellToolsCopy.Language))
            Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
                for (choice in ShellLanguage.entries) ShellPreferenceChoice(
                    strings.tools(when (choice) {
                        ShellLanguage.System -> ShellToolsCopy.System
                        ShellLanguage.English -> ShellToolsCopy.English
                        ShellLanguage.Vietnamese -> ShellToolsCopy.Vietnamese
                    }), preferences.language == choice, { onChange(preferences.copy(language = choice)) },
                    Modifier.testTag("settings-language-${choice.savedCode}"),
                )
            }
        }
        ShellSurface {
            PreferenceHeading(strings.tools(ShellToolsCopy.Motion))
            TabulaText(strings.tools(ShellToolsCopy.MotionBody), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
            Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
                for (choice in ShellMotion.entries) ShellPreferenceChoice(
                    strings.tools(if (choice == ShellMotion.System) ShellToolsCopy.System else ShellToolsCopy.Reduced),
                    preferences.motion == choice, { onChange(preferences.copy(motion = choice)) },
                    Modifier.testTag("settings-motion-${choice.savedCode}"),
                )
            }
        }
        TabulaText(strings.tools(ShellToolsCopy.SettingsScope), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
    }
}

/** Account menu entry leading to a public tool without constructing an identity or service. */
@Composable
fun ShellToolMenuCard(title: String, body: String, tag: String, onOpen: () -> Unit) {
    ShellSurface {
        PreferenceHeading(title)
        TabulaText(body, TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        ShellButton(title, false, onOpen, Modifier.testTag(tag))
    }
}

@Composable
private fun PreferenceHeading(title: String) {
    TabulaText(title, TabulaType.titleMd, Modifier.semantics { heading() })
}
