# Official Material Symbols

The `material_symbol_*.xml` shell icons are Google Material Symbols Outlined,
24px, default weight/grade and unfilled variant. The fifteen-file subset comes
from [`google/material-design-icons`](https://github.com/google/material-design-icons/tree/737e3324305806514d7909874fa1818ae1808232/symbols/android)
at commit `737e3324305806514d7909874fa1818ae1808232`:

| Shell meaning | Upstream symbol |
|---|---|
| Home | `home/materialsymbolsoutlined/home_24px.xml` |
| Library | `grid_view/materialsymbolsoutlined/grid_view_24px.xml` |
| Account and neutral avatar | `person/materialsymbolsoutlined/person_24px.xml` |
| Back | `arrow_back/materialsymbolsoutlined/arrow_back_24px.xml` |
| List view | `view_list/materialsymbolsoutlined/view_list_24px.xml` |
| Filters | `tune/materialsymbolsoutlined/tune_24px.xml` |
| Dismiss sheet | `close/materialsymbolsoutlined/close_24px.xml` |
| Open detail | `chevron_right/materialsymbolsoutlined/chevron_right_24px.xml` |
| Players | `group/materialsymbolsoutlined/group_24px.xml` |
| Duration | `schedule/materialsymbolsoutlined/schedule_24px.xml` |
| Complexity | `layers/materialsymbolsoutlined/layers_24px.xml` |
| Filter dropdown | `expand_more/materialsymbolsoutlined/expand_more_24px.xml` |
| Search | `search/materialsymbolsoutlined/search_24px.xml` |
| Selected preference | `radio_button_checked/materialsymbolsoutlined/radio_button_checked_24px.xml` |
| Unselected preference | `radio_button_unchecked/materialsymbolsoutlined/radio_button_unchecked_24px.xml` |

The path data, viewport, default dimensions and Back's `autoMirrored` flag
remain upstream values. The import removes Android's theme-only
`?attr/colorControlNormal` tint and replaces `@android:color/white` with the
equivalent opaque white mask. Compose's shared XML parser cannot resolve
external Android resources; `ShellIcon` supplies every displayed tint from the
generated Tabula tokens through Material3 `Icon`. The mask is not a palette.

Google distributes the symbols under Apache-2.0. The unmodified upstream
license is included in [`material_symbols_LICENSE.txt`](shared/src/commonMain/composeResources/files/material_symbols_LICENSE.txt)
and packaged with the shared resources. See Google's
[Material Symbols repository](https://github.com/google/material-design-icons)
and the official [Compose icon guidance](https://developer.android.com/develop/ui/compose/graphics/images/material).

Brand paths, discovery/game illustrations and permitted managed avatar
images keep their existing owners. Parent controls retain localized
semantics and touch/focus bounds; these icon children are decorative.
