package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.catalog.DiscoveryQuery
import com.loveoverflow.tabula.mobile.catalog.queryDiscovery
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaState
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.design.toTextStyle
import com.loveoverflow.tabula.mobile.localization.DiscoveryCopy
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.discovery
import com.loveoverflow.tabula.mobile.localization.discoveryCount
import com.loveoverflow.tabula.mobile.localization.discoveryDurationLimit
import com.loveoverflow.tabula.mobile.localization.discoveryMinutes
import com.loveoverflow.tabula.mobile.localization.discoveryPlayers
import com.loveoverflow.tabula.mobile.localization.discoveryRulesVersion

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

/** Search/filter input is owned above this screen so detail and Back preserve its exact values. */
@Composable
fun GamesScreen(
    catalog: DiscoveryCatalogState,
    query: DiscoveryQuery,
    strings: ShellStrings,
    onQueryChange: (DiscoveryQuery) -> Unit,
    onDetail: (DiscoveryGame) -> Unit,
    onRetry: (() -> Unit)? = null,
) {
    var showFilters by rememberSaveable { mutableStateOf(false) }
    ShellPage(strings[ShellCopy.Games], Modifier.testTag("shell-games"), displayTitle = true) {
        SearchField(query.text, strings, { onQueryChange(query.copy(text = it)) })
        if (catalog is DiscoveryCatalogState.Ready && catalog.games.isNotEmpty()) {
            CategoryFilters(catalog.games, query, strings, onQueryChange)
            ShellActionButton(strings.discovery(if (showFilters) DiscoveryCopy.HideFilters else DiscoveryCopy.Filters),
                ShellAction.Tonal, { showFilters = !showFilters }, Modifier.testTag("discovery-filter-toggle"))
            if (showFilters) CatalogFilters(catalog.games, query, strings, onQueryChange)
            if (query != DiscoveryQuery()) {
                val constraints = activeConstraints(catalog.games, query, strings)
                if (constraints.isNotBlank()) TabulaText(constraints, TabulaType.bodyMd,
                    Modifier.testTag("discovery-active-filters"), LocalTabulaColors.current.onSurfaceVariant)
                ShellActionButton(strings.discovery(DiscoveryCopy.ResetFilters), ShellAction.Text,
                    { onQueryChange(DiscoveryQuery()) }, Modifier.testTag("discovery-reset-filters"))
            }
        }
        CatalogContent(catalog, strings, onRetry) { games ->
            val results = queryDiscovery(games, query, strings.languageTag)
            TabulaText(strings.discovery(DiscoveryCopy.Results), TabulaType.titleLg, Modifier.semantics { heading() })
            TabulaText(strings.discoveryCount(results.size, games.size), TabulaType.bodyMd,
                Modifier.testTag("discovery-results-count").semantics { liveRegion = LiveRegionMode.Polite },
                LocalTabulaColors.current.onSurfaceVariant)
            if (results.isEmpty()) {
                ShellStatePanel(strings.discovery(DiscoveryCopy.NoResultsTitle), strings.discovery(DiscoveryCopy.NoResultsBody),
                    Modifier.testTag("discovery-no-results")) {
                    ShellActionButton(strings.discovery(DiscoveryCopy.ResetFilters), ShellAction.Tonal,
                        { onQueryChange(DiscoveryQuery()) }, Modifier.testTag("discovery-no-results-reset"))
                }
            } else GameCards(results, strings, onDetail)
        }
    }
}

/** Detail displays exactly the generated game metadata; startup still requires a real host binding. */
@Composable
fun DetailScreen(
    game: DiscoveryGame?,
    strings: ShellStrings,
    canLaunch: Boolean,
    onSetup: () -> Unit,
    onOpenRules: ((String) -> Unit)? = null,
    catalog: DiscoveryCatalogState? = null,
    onRetry: (() -> Unit)? = null,
) {
    ShellPage(game?.displayName(strings.languageTag) ?: strings[ShellCopy.Details], Modifier.testTag("shell-detail")) {
        if (game == null) {
            MissingEntry(catalog, strings, onRetry)
        } else {
            TabulaText(game.tagline(strings.languageTag), TabulaType.titleMd, color = LocalTabulaColors.current.onSurfaceVariant)
            DiscoveryGameCover(game.cover, Modifier.fillMaxWidth().testTag("discovery-detail-cover"))
            ShellSurface {
                GameFacts(game, strings)
                TabulaText(game.description(strings.languageTag), TabulaType.bodyLg)
                TabulaText("${strings.discovery(DiscoveryCopy.Version)} ${game.version} · ${strings.discoveryRulesVersion(game.rulesVersion)}", TabulaType.bodySm,
                    color = LocalTabulaColors.current.onSurfaceVariant)
                TabulaText("${strings.discovery(DiscoveryCopy.Rating)}: ${game.contentRatingLabel(strings.languageTag)}", TabulaType.bodyMd)
                TabulaText("${strings.discovery(DiscoveryCopy.HiddenInformation)}: ${strings.discovery(if (game.hiddenInformation) DiscoveryCopy.Yes else DiscoveryCopy.No)}", TabulaType.bodyMd)
            }
            ShellSurface {
                TabulaText(strings.discovery(DiscoveryCopy.Modes), TabulaType.titleLg, Modifier.semantics { heading() })
                if (!canLaunch) TabulaText(strings.discovery(DiscoveryCopy.NativeUnavailable), TabulaType.bodyMd,
                    Modifier.testTag("discovery-native-unavailable"))
                for (mode in game.modes) {
                    TabulaText(mode.label(strings.languageTag), TabulaType.titleMd)
                    TabulaText(mode.consequence(strings.languageTag), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
                    if (!mode.registryAvailable) {
                        if (mode.reason(strings.languageTag).isNotBlank()) TabulaText(mode.reason(strings.languageTag), TabulaType.bodyMd)
                        if (canLaunch && mode.recovery(strings.languageTag).isNotBlank()) TabulaText(mode.recovery(strings.languageTag), TabulaType.bodySm, color = LocalTabulaColors.current.onSurfaceVariant)
                    }
                }
                ShellActionButton(strings.discovery(if (canLaunch) DiscoveryCopy.Configuration else DiscoveryCopy.ViewSetup),
                    if (canLaunch) ShellAction.Filled else ShellAction.Tonal, onSetup,
                    Modifier.fillMaxWidth().testTag("shell-setup-action"))
            }
            ShellSurface {
                TabulaText(strings.discovery(DiscoveryCopy.Rules), TabulaType.titleLg, Modifier.semantics { heading() })
                val url = game.rulesUrl(strings.languageTag)
                if (url == null || onOpenRules == null) {
                    TabulaText(strings.discovery(DiscoveryCopy.RulesUnavailable), TabulaType.bodyMd)
                } else ShellActionButton(strings.discovery(DiscoveryCopy.Rules), ShellAction.Text,
                    { onOpenRules(url) }, Modifier.testTag("discovery-rules-link"))
            }
        }
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
private fun CatalogContent(
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
private fun MissingEntry(catalog: DiscoveryCatalogState?, strings: ShellStrings, onRetry: (() -> Unit)?) {
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

@Composable
private fun SearchField(text: String, strings: ShellStrings, onChange: (String) -> Unit) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val shape = RoundedCornerShape(TabulaShape.button.dp)
    Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        TabulaText(strings.discovery(DiscoveryCopy.Search), TabulaType.labelLg)
        BasicTextField(text, onChange,
            Modifier.fillMaxWidth().heightIn(min = TabulaAccessibility.minTarget.dp)
                .border(if (focused) TabulaAccessibility.focusRingWidth.dp else TabulaSpace.xxs.dp,
                    if (focused) colors.primary else colors.outline, shape)
                .background(colors.shellPaper, shape).padding(TabulaSpace.lg.dp)
                .testTag("discovery-search").semantics { contentDescription = strings.discovery(DiscoveryCopy.Search) },
            textStyle = TabulaType.bodyLg.toTextStyle(colors.onSurface), singleLine = true, interactionSource = source,
            decorationBox = { input ->
                Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), verticalAlignment = Alignment.CenterVertically) {
                    ShellIcon(ShellSymbol.Search, colors.onSurfaceVariant, size = TabulaSpace.xl.dp)
                    Box(Modifier.weight(1f)) {
                        if (text.isEmpty()) TabulaText(strings.discovery(DiscoveryCopy.SearchHint), TabulaType.bodyLg, color = colors.onSurfaceVariant)
                        input()
                    }
                }
            },
        )
        if (text.isNotEmpty()) ShellActionButton(strings.discovery(DiscoveryCopy.ClearSearch), ShellAction.Text,
            { onChange("") }, Modifier.testTag("discovery-clear-search"))
    }
}

@Composable
private fun CategoryFilters(games: List<DiscoveryGame>, query: DiscoveryQuery, strings: ShellStrings, onChange: (DiscoveryQuery) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), modifier = Modifier.testTag("discovery-categories")) {
        val categories = games.flatMap { it.categories }.distinct()
        FilterChoices(strings.discovery(DiscoveryCopy.Category), categories.map { value ->
            value to games.first { value in it.categories }.categoryLabel(value, strings.languageTag)
        }, query.category, strings, "category") { onChange(query.copy(category = it)) }
    }
}

@Composable
private fun CatalogFilters(games: List<DiscoveryGame>, query: DiscoveryQuery, strings: ShellStrings, onChange: (DiscoveryQuery) -> Unit) {
    ShellSurface(Modifier.testTag("discovery-filters")) {
        FilterChoices(strings.discovery(DiscoveryCopy.Players), games.flatMap { it.players }.distinct().sorted().map {
            it.toString() to strings.discoveryPlayers(listOf(it))
        }, query.players?.toString(), strings, "player") { onChange(query.copy(players = it?.toIntOrNull())) }
        FilterChoices(strings.discovery(DiscoveryCopy.Duration), games.map { it.maxMinutes }.distinct().sorted().map {
            it.toString() to strings.discoveryDurationLimit(it)
        }, query.maxMinutes?.toString(), strings, "duration") { onChange(query.copy(maxMinutes = it?.toIntOrNull())) }
        FilterChoices(strings.discovery(DiscoveryCopy.Complexity), games.distinctBy { it.complexity }.map {
            it.complexity to it.complexityLabel(strings.languageTag)
        }, query.complexity, strings, "complexity") { onChange(query.copy(complexity = it)) }
    }
}

@Composable
private fun FilterChoices(title: String, choices: List<Pair<String, String>>, value: String?, strings: ShellStrings,
    axis: String, onChange: (String?) -> Unit,
) {
    TabulaText(title, TabulaType.titleMd)
    FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        FilterChoice(strings.discovery(DiscoveryCopy.All), value == null, { onChange(null) }, "discovery-filter-$axis-all")
        for ((id, label) in choices) FilterChoice(label, value == id, { onChange(id) }, "discovery-filter-$axis-$id")
    }
}

@Composable
private fun FilterChoice(label: String, selected: Boolean, onClick: () -> Unit, tag: String) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val hovered by source.collectIsHoveredAsState()
    val pressed by source.collectIsPressedAsState()
    val shape = RoundedCornerShape(TabulaShape.chip.dp)
    val foreground = if (selected) colors.onPrimary else colors.onSurface
    val background = if (selected) colors.primary else colors.shellPaper
    val stateLayer = when {
        pressed -> TabulaState.press
        focused -> TabulaState.focus
        hovered -> TabulaState.hover
        else -> 0f
    }
    Box(Modifier.widthIn(min = TabulaAccessibility.minTarget.dp).heightIn(min = TabulaAccessibility.minTarget.dp)
        .border(if (focused) TabulaAccessibility.focusRingWidth.dp else TabulaSpace.xxs.dp,
            if (focused || selected) colors.primary else colors.outline, shape)
        .background(foreground.copy(alpha = stateLayer).compositeOver(background), shape)
        .selectable(selected, source, indication = null, role = Role.RadioButton, onClick = onClick)
        .padding(horizontal = TabulaSpace.md.dp, vertical = TabulaSpace.sm.dp).testTag(tag), contentAlignment = Alignment.Center) {
        TabulaText(label, TabulaType.labelLg, color = foreground)
    }
}

private fun activeConstraints(games: List<DiscoveryGame>, query: DiscoveryQuery, strings: ShellStrings): String = listOfNotNull(
    query.category?.let { id -> games.firstOrNull { id in it.categories }?.categoryLabel(id, strings.languageTag) },
    query.players?.let { strings.discoveryPlayers(listOf(it)) },
    query.maxMinutes?.let { strings.discoveryDurationLimit(it) },
    query.complexity?.let { id -> games.firstOrNull { it.complexity == id }?.complexityLabel(strings.languageTag) },
).joinToString(" · ")
