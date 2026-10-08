package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.catalog.*
import com.loveoverflow.tabula.mobile.design.*
import com.loveoverflow.tabula.mobile.localization.*

/** Library presentation is public local state; selecting a card mounts only its metadata sheet (ADR-0045). */
@Composable
fun GamesScreen(
    catalog: DiscoveryCatalogState,
    query: DiscoveryQuery,
    strings: ShellStrings,
    onQueryChange: (DiscoveryQuery) -> Unit,
    onDetail: (DiscoveryGame) -> Unit,
    onRetry: (() -> Unit)? = null,
) {
    var grid by rememberSaveable { mutableStateOf(false) }
    var showFilters by remember { mutableStateOf(false) }
    val games = (catalog as? DiscoveryCatalogState.Ready)?.games.orEmpty()
    ShellPage(strings[ShellCopy.Games], Modifier.testTag("shell-games"), titleContent = {}) {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val filter: @Composable () -> Unit = {
                LibraryIconAction(strings.discovery(DiscoveryCopy.Filters), ShellSymbol.Filter,
                    { showFilters = true }, Modifier.testTag("discovery-filter-toggle"), tonal = true)
            }
            if (LocalDensity.current.fontScale > 1.3f || maxWidth < 300.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
                    LibrarySearch(query.text, strings) { onQueryChange(query.copy(text = it)) }
                    filter()
                }
            } else Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { LibrarySearch(query.text, strings) { onQueryChange(query.copy(text = it)) } }
                filter()
            }
        }
        if (query.text.isNotEmpty()) ShellActionButton(strings.discovery(DiscoveryCopy.ClearSearch), ShellAction.Text,
            { onQueryChange(query.copy(text = "")) }, Modifier.heightIn(min = 48.dp).testTag("discovery-clear-search"))
        val constraints = activeConstraints(games, query, strings)
        if (constraints.isNotEmpty()) {
            TabulaText(constraints, TabulaType.bodyMd, Modifier.testTag("discovery-active-filters"), LocalTabulaColors.current.onSurfaceVariant)
            ShellActionButton(strings.discovery(DiscoveryCopy.ResetFilters), ShellAction.Text,
                { onQueryChange(DiscoveryQuery(text = query.text)) }, Modifier.heightIn(min = 48.dp).testTag("discovery-reset-filters"))
        }
        CatalogContent(catalog, strings, onRetry) { ready ->
            val results = queryDiscovery(ready, query, strings.languageTag)
            FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
                Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
                    TabulaText(strings.discovery(DiscoveryCopy.Results), TabulaType.titleMd, Modifier.semantics { heading() })
                    TabulaText(strings.discoveryCount(results.size, ready.size), TabulaType.bodyMd,
                        Modifier.testTag("discovery-results-count").semantics { liveRegion = LiveRegionMode.Polite }, LocalTabulaColors.current.onSurfaceVariant)
                }
                Row(Modifier.selectableGroup().background(LocalTabulaColors.current.shellNote, RoundedCornerShape(TabulaShape.button.dp))) {
                    LibraryViewChoice(strings.discovery(DiscoveryCopy.ListView), ShellSymbol.List, !grid, { grid = false }, "discovery-view-list")
                    LibraryViewChoice(strings.discovery(DiscoveryCopy.GridView), ShellSymbol.Library, grid, { grid = true }, "discovery-view-grid")
                }
            }
            if (results.isEmpty()) ShellStatePanel(strings.discovery(DiscoveryCopy.NoResultsTitle), strings.discovery(DiscoveryCopy.NoResultsBody),
                Modifier.testTag("discovery-no-results")) {
                ShellActionButton(strings.discovery(DiscoveryCopy.ResetFilters), ShellAction.Tonal,
                    { onQueryChange(DiscoveryQuery()) }, Modifier.heightIn(min = 48.dp).testTag("discovery-no-results-reset"))
            } else LibraryCards(results, strings, grid, onDetail)
        }
    }
    if (showFilters) LibraryFilters(games, query, strings, onDismiss = { showFilters = false }, onApply = {
        onQueryChange(it)
        showFilters = false
    })
}

@Composable
private fun LibrarySearch(text: String, strings: ShellStrings, onChange: (String) -> Unit) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val shape = RoundedCornerShape(TabulaShape.button.dp)
    BasicTextField(text, onChange,
        Modifier.fillMaxWidth().heightIn(min = 52.dp)
            .background(colors.shellPaper, shape)
            .border(if (focused) TabulaAccessibility.focusRingWidth.dp else 1.dp, if (focused) colors.primary else colors.outline, shape)
            .padding(TabulaSpace.md.dp).testTag("discovery-search").semantics { contentDescription = strings.discovery(DiscoveryCopy.Search) },
        textStyle = TabulaType.bodyLg.toTextStyle(colors.onSurface), singleLine = true, interactionSource = source,
        decorationBox = { input ->
            Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), verticalAlignment = Alignment.CenterVertically) {
                ShellIcon(ShellSymbol.Search, colors.onSurfaceVariant, size = 24.dp)
                Box(Modifier.weight(1f)) {
                    if (text.isEmpty()) TabulaText(strings.discovery(DiscoveryCopy.Search), TabulaType.bodyLg, color = colors.onSurfaceVariant)
                    input()
                }
            }
        },
    )
}

@Composable
private fun LibraryViewChoice(label: String, icon: ShellSymbol, selected: Boolean, onClick: () -> Unit, tag: String) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val shape = RoundedCornerShape(TabulaShape.button.dp)
    Box(Modifier.size(56.dp).padding(4.dp).background(libraryStateColor(source, if (selected) colors.shellPaper else colors.shellNote, colors.primary), shape)
        .border(if (focused) TabulaAccessibility.focusRingWidth.dp else 1.dp,
            if (focused || (selected && colors.shellPaper == colors.shellCanvas)) colors.primary else colors.shellNote, shape)
        .selectable(selected, source, indication = null, role = Role.RadioButton, onClick = onClick)
        .testTag(tag).semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        ShellIcon(icon, if (selected) colors.primary else colors.onSurfaceVariant)
    }
}

/** Shared square-icon list/grid; large text and narrow phones release every card's reading width. */
@Composable
private fun LibraryCards(games: List<DiscoveryGame>, strings: ShellStrings, grid: Boolean, onDetail: (DiscoveryGame) -> Unit) {
    val fontScale = LocalDensity.current.fontScale
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val columns = if (!grid || fontScale > 1.3f || maxWidth < 340.dp) 1 else if (maxWidth < 900.dp) 2 else 3
        Column(Modifier.testTag("discovery-library-cards"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            for (chunk in games.chunked(columns)) Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                for (game in chunk) key(game.id) {
                    Box(Modifier.weight(1f)) { LibraryCard(game, strings, grid, fontScale > 1.3f, onDetail) }
                }
                repeat(columns - chunk.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
private fun LibraryCard(game: DiscoveryGame, strings: ShellStrings, grid: Boolean, largeText: Boolean, onDetail: (DiscoveryGame) -> Unit) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val shape = RoundedCornerShape(TabulaShape.card.dp)
    Box(Modifier.fillMaxWidth().testTag("discovery-card-${game.id}")) {
    Column(Modifier.fillMaxWidth().background(libraryStateColor(source, colors.shellPaper, colors.primary), shape)
        .border(if (focused) TabulaAccessibility.focusRingWidth.dp else 1.dp,
            if (focused) colors.primary else if (colors.shellPaper == colors.shellCanvas) colors.outline else colors.shellPaper, shape)
        .clickable(interactionSource = source, indication = null, role = Role.Button, onClick = { onDetail(game) })
        .testTag("shell-details-${game.id}").semantics { contentDescription = strings.details(game.displayName(strings.languageTag)) }
        .padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (grid || largeText) {
            DiscoveryGameIcon(game, Modifier.testTag("discovery-card-cover-${game.id}"), size = 72.dp)
            LibraryCardHeading(game, strings, showTagline = !grid)
            LibraryFacts(game, strings, vertical = grid)
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                TabulaText(strings[ShellCopy.Details], TabulaType.labelLg, color = colors.primary)
                ShellIcon(ShellSymbol.Chevron, colors.primary)
            }
        } else Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Top) {
            DiscoveryGameIcon(game, Modifier.testTag("discovery-card-cover-${game.id}"), size = 72.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                LibraryCardHeading(game, strings, showTagline = true)
                LibraryFacts(game, strings)
            }
            ShellIcon(ShellSymbol.Chevron, colors.primary, size = 16.dp)
        }
    }
    }
}

@Composable
private fun LibraryCardHeading(game: DiscoveryGame, strings: ShellStrings, showTagline: Boolean) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            TabulaText(game.displayName(strings.languageTag), TabulaType.titleLg)
            if (game.planned) PlannedBadge(strings)
        }
        if (showTagline) TabulaText(game.tagline(strings.languageTag), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
    }
}

@Composable
private fun PlannedBadge(strings: ShellStrings) {
    val colors = LocalTabulaColors.current
    TabulaText(strings.discovery(DiscoveryCopy.ComingSoon), TabulaType.labelMd,
        Modifier.background(colors.primary.copy(alpha = TabulaState.focus).compositeOver(colors.shellPaper), RoundedCornerShape(TabulaShape.chip.dp))
            .padding(horizontal = 8.dp, vertical = 4.dp), colors.primary)
}

@Composable
private fun LibraryFacts(game: DiscoveryGame, strings: ShellStrings, vertical: Boolean = false) {
    val facts = listOf(ShellSymbol.People to strings.discoveryPlayers(game.players),
        ShellSymbol.Clock to strings.discoveryMinutes(game.minMinutes, game.maxMinutes),
        ShellSymbol.Layers to game.complexityLabel(strings.languageTag))
    if (vertical) Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        facts.forEach { (icon, label) -> LibraryFact(icon, label) }
    } else FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        facts.forEach { (icon, label) -> LibraryFact(icon, label) }
    }
}

@Composable
private fun LibraryFact(icon: ShellSymbol, label: String) {
    val colors = LocalTabulaColors.current
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        ShellIcon(icon, colors.onSurfaceVariant, size = 18.dp)
        TabulaText(label, TabulaType.bodyMd, color = colors.onSurfaceVariant)
    }
}

/** Modal dismissal cancels the draft; only Apply publishes it to the route's current query. */
@Composable
private fun LibraryFilters(games: List<DiscoveryGame>, query: DiscoveryQuery, strings: ShellStrings, onDismiss: () -> Unit, onApply: (DiscoveryQuery) -> Unit) {
    var draft by remember { mutableStateOf(query) }
    LibrarySheet(strings.discovery(DiscoveryCopy.Filters), strings, "discovery-filters", "discovery-filter-cancel", onDismiss, closeLabel = strings.discovery(DiscoveryCopy.CancelFilters)) {
        Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).testTag("discovery-filter-scroll"),
            verticalArrangement = Arrangement.spacedBy(16.dp)) {
            FilterDropdown(strings.discovery(DiscoveryCopy.Category), games.flatMap { it.categories }.distinct().map { id ->
                id to games.first { id in it.categories }.categoryLabel(id, strings.languageTag)
            }, draft.category, strings, "category") { draft = draft.copy(category = it) }
            FilterDropdown(strings.discovery(DiscoveryCopy.Players), games.flatMap { it.players }.distinct().sorted().map {
                it.toString() to strings.discoveryPlayers(listOf(it))
            }, draft.players?.toString(), strings, "player") { draft = draft.copy(players = it?.toIntOrNull()) }
            FilterDropdown(strings.discovery(DiscoveryCopy.Duration), games.map { it.maxMinutes }.distinct().sorted().map {
                it.toString() to strings.discoveryDurationLimit(it)
            }, draft.maxMinutes?.toString(), strings, "duration") { draft = draft.copy(maxMinutes = it?.toIntOrNull()) }
            FilterDropdown(strings.discovery(DiscoveryCopy.Complexity), games.distinctBy { it.complexity }.map {
                it.complexity to it.complexityLabel(strings.languageTag)
            }, draft.complexity, strings, "complexity") { draft = draft.copy(complexity = it) }
            TabulaText(strings.discovery(DiscoveryCopy.FilterHint), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        }
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val reset: @Composable (Modifier) -> Unit = { modifier ->
                ShellActionButton(strings.discovery(DiscoveryCopy.ResetFilters), ShellAction.Tonal,
                    { draft = DiscoveryQuery(text = query.text) }, modifier.heightIn(min = 48.dp).testTag("discovery-filter-reset"))
            }
            val apply: @Composable (Modifier) -> Unit = { modifier ->
                ShellActionButton(strings.discovery(DiscoveryCopy.ApplyFilters), ShellAction.Filled,
                    { onApply(draft) }, modifier.heightIn(min = 48.dp).testTag("discovery-filter-apply"))
            }
            if (LocalDensity.current.fontScale > 1.3f) Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                reset(Modifier.fillMaxWidth())
                apply(Modifier.fillMaxWidth())
            } else Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                reset(Modifier.weight(1f))
                apply(Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun FilterDropdown(title: String, choices: List<Pair<String, String>>, value: String?, strings: ShellStrings, axis: String, onChange: (String?) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    val colors = LocalTabulaColors.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        TabulaText(title, TabulaType.labelLg)
        Box {
            val label = choices.firstOrNull { it.first == value }?.second ?: strings.discovery(DiscoveryCopy.All)
            LibraryIconAction(label, ShellSymbol.Expand,
                { expanded = true }, Modifier.fillMaxWidth().testTag("discovery-filter-$axis"), outlined = true, description = "$title, $label")
            DropdownMenu(expanded, onDismissRequest = { expanded = false }, containerColor = colors.shellPaper,
                modifier = Modifier.heightIn(max = 320.dp).widthIn(max = 280.dp).selectableGroup()) {
                for ((id, label) in listOf("all" to strings.discovery(DiscoveryCopy.All)) + choices) {
                    ShellPreferenceChoice(label, if (id == "all") value == null else value == id,
                        { onChange(if (id == "all") null else id); expanded = false },
                        Modifier.heightIn(min = 48.dp).testTag("discovery-filter-$axis-$id"))
                }
            }
        }
    }
}

/** Detail retains public route identity while the caller stays mounted beneath the sheet. */
@Composable
fun DetailScreen(game: DiscoveryGame?, strings: ShellStrings, canLaunch: Boolean, onSetup: () -> Unit,
    onDismiss: () -> Unit, onOpenRules: ((String) -> Unit)? = null, catalog: DiscoveryCatalogState? = null, onRetry: (() -> Unit)? = null,
) {
    var showModes by remember { mutableStateOf(false) }
    LibrarySheet(strings.discovery(DiscoveryCopy.DetailTitle), strings, "shell-detail", "shell-back", onDismiss) {
        Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).testTag("discovery-detail-scroll"),
            verticalArrangement = Arrangement.spacedBy(16.dp)) {
            if (game == null) MissingEntry(catalog, strings, onRetry) else {
                val heading: @Composable () -> Unit = {
                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        TabulaText(game.displayName(strings.languageTag), TabulaType.headlineSm, Modifier.semantics { heading() })
                        if (game.planned) PlannedBadge(strings)
                        TabulaText(game.categories.joinToString(" · ") { game.categoryLabel(it, strings.languageTag) }, TabulaType.bodyMd,
                            color = LocalTabulaColors.current.onSurfaceVariant)
                    }
                }
                if (LocalDensity.current.fontScale > 1.3f) {
                    DiscoveryGameIcon(game, Modifier.testTag("discovery-detail-cover"), size = 88.dp)
                    heading()
                } else Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    DiscoveryGameIcon(game, Modifier.testTag("discovery-detail-cover"), size = 88.dp)
                    Box(Modifier.weight(1f)) { heading() }
                }
                LibraryFacts(game, strings)
                TabulaText(game.description(strings.languageTag), TabulaType.bodyLg)
                DetailFact(strings.discovery(DiscoveryCopy.Rating), game.contentRatingLabel(strings.languageTag))
                DetailFact(strings.discovery(DiscoveryCopy.HiddenInformation), strings.discovery(if (game.hiddenInformation) DiscoveryCopy.Yes else DiscoveryCopy.No))
                DetailFact(strings.discovery(DiscoveryCopy.Version), game.version)
                TabulaText(strings.discoveryRulesVersion(game.rulesVersion), TabulaType.bodySm, color = LocalTabulaColors.current.onSurfaceVariant)
                if (!canLaunch || game.planned) ShellSurface(Modifier.testTag("discovery-native-unavailable")) {
                    TabulaText(strings.discovery(if (game.planned) DiscoveryCopy.PlannedUnavailable else DiscoveryCopy.NativeUnavailable), TabulaType.bodyMd)
                }
                if (!game.planned) {
                    if (game.modes.isNotEmpty()) ShellActionButton(strings.discovery(DiscoveryCopy.Modes), ShellAction.Text,
                        { showModes = !showModes }, Modifier.heightIn(min = 48.dp).testTag("discovery-detail-modes").semantics { selected = showModes })
                    if (showModes) for (mode in game.modes) {
                        TabulaText(mode.label(strings.languageTag), TabulaType.titleMd)
                        TabulaText(mode.consequence(strings.languageTag), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
                        if (!mode.registryAvailable && mode.reason(strings.languageTag).isNotBlank()) TabulaText(mode.reason(strings.languageTag), TabulaType.bodyMd)
                    }
                    ShellActionButton(strings.discovery(if (canLaunch) DiscoveryCopy.Configuration else DiscoveryCopy.ViewSetup), ShellAction.Tonal,
                        onSetup, Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("shell-setup-action"))
                }
                val rules = game.rulesUrl(strings.languageTag)
                if (rules != null && onOpenRules != null) ShellActionButton(strings.discovery(DiscoveryCopy.Rules), ShellAction.Text,
                    { onOpenRules(rules) }, Modifier.heightIn(min = 48.dp).testTag("discovery-rules-link"))
                else TabulaText(strings.discovery(DiscoveryCopy.RulesUnavailable), TabulaType.bodyMd)
            }
        }
        ShellActionButton(strings[ShellCopy.Back], ShellAction.Tonal, onDismiss,
            Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("discovery-detail-dismiss"))
    }
}

@Composable
private fun DetailFact(label: String, value: String) {
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        TabulaText(label, TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        TabulaText(value, TabulaType.bodyMd)
    }
}

/** Official modal owns focus, outside dismissal and Back ordering; all visible roles use Tabula tokens. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LibrarySheet(title: String, strings: ShellStrings, tag: String, closeTag: String, onDismiss: () -> Unit, closeLabel: String = strings[ShellCopy.Back], content: @Composable ColumnScope.() -> Unit) {
    val colors = LocalTabulaColors.current
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.shellPaper, contentColor = colors.onSurface, scrimColor = colors.onSurface.copy(alpha = TabulaState.disabledContent),
        tonalElevation = 0.dp, shape = RoundedCornerShape(topStart = TabulaShape.card.dp, topEnd = TabulaShape.card.dp), dragHandle = null,
    ) {
        BoxWithConstraints(Modifier.fillMaxWidth().testTag(tag)) {
            Column(Modifier.fillMaxWidth().heightIn(max = maxHeight * 0.9f).padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TabulaText(title, TabulaType.titleLg, Modifier.weight(1f).semantics { heading() })
                    LibraryIconAction(closeLabel, ShellSymbol.Close, onDismiss, Modifier.testTag(closeTag))
                }
                content()
            }
        }
    }
}

@Composable
private fun LibraryIconAction(label: String, icon: ShellSymbol, onClick: () -> Unit, modifier: Modifier = Modifier, tonal: Boolean = false, outlined: Boolean = false, description: String = label) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val shape = RoundedCornerShape(TabulaShape.button.dp)
    Row(modifier.heightIn(min = 52.dp).widthIn(min = 48.dp)
        .background(libraryStateColor(source, if (tonal) colors.containerHigh else colors.shellPaper, colors.primary), shape)
        .border(if (focused) TabulaAccessibility.focusRingWidth.dp else 1.dp,
            if (focused) colors.primary else if (outlined || colors.shellPaper == colors.shellCanvas) colors.outline else colors.shellPaper, shape)
        .clickable(interactionSource = source, indication = null, role = Role.Button, onClick = onClick)
        .semantics { contentDescription = description }.padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (!outlined) ShellIcon(icon, colors.primary)
        if (tonal || outlined) TabulaText(label, if (outlined) TabulaType.bodyLg else TabulaType.labelLg,
            if (outlined) Modifier.weight(1f) else Modifier, if (outlined) colors.onSurfaceVariant else colors.primary)
        if (outlined) ShellIcon(icon, colors.onSurfaceVariant)
    }
}

private fun activeConstraints(games: List<DiscoveryGame>, query: DiscoveryQuery, strings: ShellStrings): String = listOfNotNull(
    query.category?.let { id -> games.firstOrNull { id in it.categories }?.categoryLabel(id, strings.languageTag) },
    query.players?.let { strings.discoveryPlayers(listOf(it)) }, query.maxMinutes?.let { strings.discoveryDurationLimit(it) },
    query.complexity?.let { id -> games.firstOrNull { it.complexity == id }?.complexityLabel(strings.languageTag) },
).joinToString(" · ")

/** Immediate state layers also work when reduced motion is requested. */
@Composable
private fun libraryStateColor(source: MutableInteractionSource, background: Color, foreground: Color): Color {
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    val hovered by source.collectIsHoveredAsState()
    val alpha = when { pressed -> TabulaState.press; focused -> TabulaState.focus; hovered -> TabulaState.hover; else -> 0f }
    return foreground.copy(alpha = alpha).compositeOver(background)
}
