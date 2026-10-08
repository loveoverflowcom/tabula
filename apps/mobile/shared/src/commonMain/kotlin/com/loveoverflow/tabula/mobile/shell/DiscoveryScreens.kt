package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.localization.DiscoveryCopy
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.discovery
import com.loveoverflow.tabula.mobile.localization.discoveryMinutes
import com.loveoverflow.tabula.mobile.localization.discoveryPlayers

/** Metadata-only discovery Home; no local history or featured policy is fabricated (I-9). */
@Composable
fun HomeScreen(
    catalog: DiscoveryCatalogState,
    strings: ShellStrings,
    onBrowse: () -> Unit,
    onDetail: (DiscoveryGame) -> Unit,
    onRetry: (() -> Unit)? = null,
) {
    val colors = LocalTabulaColors.current
    ShellPage(strings.discovery(DiscoveryCopy.HomeHeading), Modifier.testTag("shell-home"), displayTitle = true,
        titleContent = {
            Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
                TabulaText(strings[ShellCopy.HomeHeading], TabulaType.labelLg, color = colors.onSurfaceVariant)
                ShellDisplayHeading(strings.discovery(DiscoveryCopy.HomeHeading))
                TabulaText(strings[ShellCopy.HomeIntro], TabulaType.bodyLg, color = colors.onSurfaceVariant)
            }
        },
    ) {
        DiscoveryHero(strings, onBrowse)
        ShellDisplayHeading(strings.discovery(DiscoveryCopy.CatalogHeading))
        CatalogContent(catalog, strings, onRetry) { games ->
            // Preserve registry order and show every entry; no invented ranking or featured tag.
            GameCards(games, strings, onDetail)
        }
        ShellActionButton(strings.discovery(DiscoveryCopy.BrowseAll), ShellAction.Text, onBrowse, Modifier.fillMaxWidth())
    }
}

@Composable
private fun DiscoveryHero(strings: ShellStrings, onBrowse: () -> Unit) {
    val fontScale = LocalDensity.current.fontScale
    ShellSurface(Modifier.testTag("discovery-hero"), hero = true) {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val compact = maxWidth < 620.dp
            if (!compact && fontScale <= 1.3f) {
                Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.xl.dp), verticalAlignment = Alignment.CenterVertically) {
                    HeroCopy(strings, onBrowse, Modifier.weight(1f))
                    DiscoveryHeroArt(Modifier.weight(1f))
                }
            } else if (fontScale <= 1.3f) {
                // Only the decorative art shares the heading row; prose and the target keep full width.
                Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp), verticalAlignment = Alignment.CenterVertically) {
                        HeroHeading(strings, compact = true, Modifier.weight(1f))
                        DiscoveryHeroArt(Modifier.width((TabulaSpace.xxxxxxl + TabulaSpace.xxxl).dp))
                    }
                    HeroBodyAndAction(strings, onBrowse)
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp)) {
                    // Large text gets the entire reading width; the illustration stays secondary.
                    HeroCopy(strings, onBrowse, compact = compact)
                    DiscoveryHeroArt(Modifier.width((TabulaSpace.xxxxxxl + TabulaSpace.xxxl).dp).align(Alignment.End))
                }
            }
        }
    }
}

@Composable
private fun HeroCopy(strings: ShellStrings, onBrowse: () -> Unit, modifier: Modifier = Modifier, compact: Boolean = false) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp)) {
        HeroHeading(strings, compact)
        HeroBodyAndAction(strings, onBrowse)
    }
}

@Composable
private fun HeroHeading(strings: ShellStrings, compact: Boolean, modifier: Modifier = Modifier) {
    val colors = LocalTabulaColors.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        TabulaText(strings.discovery(DiscoveryCopy.HeroEyebrow), TabulaType.labelLg, color = colors.shellOnHero)
        TabulaText(strings.discovery(DiscoveryCopy.HeroHeading), if (compact) TabulaType.shellDisplay else TabulaType.displaySm,
            Modifier.semantics { heading() }, colors.shellOnHero)
    }
}

@Composable
private fun HeroBodyAndAction(strings: ShellStrings, onBrowse: () -> Unit) {
    val colors = LocalTabulaColors.current
    Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp)) {
        TabulaText(strings.discovery(DiscoveryCopy.HeroBody), TabulaType.bodyLg, color = colors.shellOnHero)
        ShellActionButton(strings[ShellCopy.BrowseGames], ShellAction.Filled, onBrowse,
            Modifier.fillMaxWidth().testTag("shell-browse-games"))
    }
}

/** Read-only module defaults cannot claim a native configuration adapter that does not exist. */
@Composable
fun SetupScreen(
    game: DiscoveryGame?,
    strings: ShellStrings,
    canLaunch: Boolean,
    onPlay: () -> Unit,
    catalog: DiscoveryCatalogState? = null,
    onRetry: (() -> Unit)? = null,
) {
    ShellPage(game?.let { strings.setup(it.displayName(strings.languageTag)) } ?: strings[ShellCopy.SetupTitle], Modifier.testTag("shell-setup")) {
        if (game == null) {
            MissingEntry(catalog, strings, onRetry)
        } else if (game.planned) {
            ShellStatePanel(strings.discovery(DiscoveryCopy.ComingSoon), strings.discovery(DiscoveryCopy.PlannedUnavailable),
                Modifier.testTag("discovery-planned-setup-unavailable"))
        } else {
            ShellSurface {
                TabulaText(strings[ShellCopy.LocalMode], TabulaType.titleLg, Modifier.semantics { heading() })
                GameFacts(game, strings)
                TabulaText(strings.discovery(DiscoveryCopy.ConfigurationDefaults), TabulaType.bodyMd)
                if (!canLaunch) TabulaText(strings.discovery(DiscoveryCopy.NativeUnavailable), TabulaType.bodyMd,
                    Modifier.testTag("shell-native-unavailable"))
                ShellActionButton(strings[ShellCopy.StartLocalGame], ShellAction.Filled, onPlay,
                    Modifier.fillMaxWidth().testTag("shell-start-local"), enabled = canLaunch)
            }
            ShellSurface {
                TabulaText(strings.discovery(DiscoveryCopy.Configuration), TabulaType.titleLg, Modifier.semantics { heading() })
                TabulaText(strings.discovery(DiscoveryCopy.ConfigurationUnavailable), TabulaType.bodyMd)
                if (game.fields.isEmpty()) TabulaText(strings.discovery(DiscoveryCopy.NoConfiguration), TabulaType.bodyMd)
                for (field in game.visibleFields(game.defaultDraft())) {
                    TabulaText(field.label(strings.languageTag), TabulaType.titleMd)
                    val default = field.default
                    val choice = field.choices.firstOrNull { it.value == default }
                    val value = choice?.label(strings.languageTag) ?: default
                    if (value.isNotBlank()) TabulaText(value, TabulaType.bodyLg)
                    val hint = field.hint(strings.languageTag)
                    if (hint.isNotBlank()) TabulaText(hint, TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
                }
            }
            ShellStatePanel(strings.discovery(DiscoveryCopy.OnlineUnavailable), strings[ShellCopy.HomeStatus],
                Modifier.testTag("shell-online-unavailable"))
        }
    }
}

@Composable
internal fun CatalogContent(
    catalog: DiscoveryCatalogState,
    strings: ShellStrings,
    onRetry: (() -> Unit)?,
    ready: @Composable (List<DiscoveryGame>) -> Unit,
) {
    when (catalog) {
        DiscoveryCatalogState.Loading -> ShellLoading(strings, Modifier.testTag("discovery-catalog-loading"))
        DiscoveryCatalogState.Unavailable -> ShellStatePanel(strings.discovery(DiscoveryCopy.CatalogUnavailableTitle),
            strings.discovery(DiscoveryCopy.CatalogUnavailableBody), Modifier.testTag("shell-catalog-unavailable"))
        is DiscoveryCatalogState.Error -> ShellStatePanel(strings.discovery(DiscoveryCopy.CatalogErrorTitle),
            strings.discovery(DiscoveryCopy.CatalogErrorBody), Modifier.testTag("discovery-catalog-error"), error = true,
            action = onRetry?.let { retry -> {
                ShellActionButton(strings[ShellCopy.Retry], ShellAction.Filled, retry, Modifier.testTag("discovery-catalog-retry"))
            } })
        is DiscoveryCatalogState.Ready -> if (catalog.games.isEmpty()) {
            ShellStatePanel(strings.discovery(DiscoveryCopy.EmptyTitle), strings.discovery(DiscoveryCopy.EmptyBody), Modifier.testTag("discovery-empty"))
        } else ready(catalog.games)
    }
}

@Composable
internal fun MissingEntry(catalog: DiscoveryCatalogState?, strings: ShellStrings, onRetry: (() -> Unit)?) {
    if (catalog != null && catalog !is DiscoveryCatalogState.Ready) CatalogContent(catalog, strings, onRetry) { }
    else UnknownGame(strings)
}

@Composable
private fun UnknownGame(strings: ShellStrings) = ShellStatePanel(
    strings[ShellCopy.DetailUnavailableTitle], strings[ShellCopy.DetailUnavailable], Modifier.testTag("discovery-game-not-found"),
)

@Composable
private fun GameFacts(game: DiscoveryGame, strings: ShellStrings) {
    val colors = LocalTabulaColors.current
    FlowRow(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp), verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        TabulaText(strings.discoveryPlayers(game.players), TabulaType.bodyMd, color = colors.onSurfaceVariant)
        TabulaText(strings.discoveryMinutes(game.minMinutes, game.maxMinutes), TabulaType.bodyMd, color = colors.onSurfaceVariant)
        TabulaText(game.complexityLabel(strings.languageTag), TabulaType.bodyMd, color = colors.onSurfaceVariant)
    }
    if (game.categories.isNotEmpty()) TabulaText(game.categories.joinToString(" · ") { game.categoryLabel(it, strings.languageTag) },
        TabulaType.labelMd, color = colors.primary)
}

@Composable
private fun GameCards(games: List<DiscoveryGame>, strings: ShellStrings, onDetail: (DiscoveryGame) -> Unit) {
    val fontScale = LocalDensity.current.fontScale
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val columns = if (fontScale > 1.3f || maxWidth < 620.dp) 1 else if (maxWidth < 1_000.dp) 2 else 3
        val compactCards = maxWidth < 620.dp && fontScale <= 1.3f
        Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp)) {
            for (chunk in games.chunked(columns)) {
                Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp)) {
                    for (game in chunk) Box(Modifier.weight(1f)) { GameCard(game, strings, onDetail, compactCards) }
                    repeat(columns - chunk.size) { Box(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun GameCard(game: DiscoveryGame, strings: ShellStrings, onDetail: (DiscoveryGame) -> Unit, compact: Boolean) {
    val shape = RoundedCornerShape(TabulaShape.card.dp)
    val colors = LocalTabulaColors.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val thumbnailWidth = if (maxWidth < 340.dp) (TabulaSpace.xxxxxxl + TabulaSpace.xl).dp
            else (TabulaSpace.xxxxl * 3).dp
        Column(Modifier.fillMaxWidth().background(colors.shellPaper, shape)
            .border(TabulaSpace.xxs.dp, if (colors.shellPaper == colors.shellCanvas) colors.outline else colors.shellPaper, shape).testTag("discovery-card-${game.id}")) {
            if (!compact) DiscoveryGameCover(game.cover, Modifier.fillMaxWidth())
            Column(Modifier.fillMaxWidth().padding(TabulaSpace.lg.dp), verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
                if (compact) {
                    Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp), verticalAlignment = Alignment.CenterVertically) {
                        // The artwork owns one small clip; a full-card radius would remove much of a short thumbnail.
                        DiscoveryGameCover(game.cover, Modifier.width(thumbnailWidth).testTag("discovery-card-cover-${game.id}"),
                            shape = RoundedCornerShape(TabulaShape.xs.dp))
                        GameCardHeading(game, strings, Modifier.weight(1f))
                    }
                } else GameCardHeading(game, strings)
                GameFacts(game, strings)
                ShellActionButton(strings.details(game.displayName(strings.languageTag)), ShellAction.Text,
                    { onDetail(game) }, Modifier.fillMaxWidth().testTag("shell-details-${game.id}"))
            }
        }
    }
}

@Composable
private fun GameCardHeading(game: DiscoveryGame, strings: ShellStrings, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        TabulaText(game.displayName(strings.languageTag), TabulaType.titleLg, Modifier.semantics { heading() })
        TabulaText(game.tagline(strings.languageTag), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
    }
}
