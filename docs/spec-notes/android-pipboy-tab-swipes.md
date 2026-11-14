# Pip-Boy navigation swipes

- [documented] The owner requested left/right main-section and subtab navigation,
  separate from vertical scrolling, item assignment, scrollbars and arm gestures.
- [documented] Existing `TouchItems::Point` keeps the original window-logical touch
  position, while `flat_pointer`/model projection produce the menu's canvas position.
  The 48-logical-pixel commit threshold uses the former; canvas points choose only
  the original painted navigation region.
- [documented] Navigation may start only on an actual painted main header/title or
  subtab region. A pending ordinary tap keeps the existing pointer route. Once motion
  locks horizontal, pointer actions cancel; only a sufficient released swipe emits
  the existing typed `PipBoyInput`. Vertical motion, cancellation, a changed session
  or geometry, lifecycle/input capture and a long pending hold produce no navigation.
- [documented] Body rows and scrollbar regions are never navigation regions. Their
  existing vertical scroll and deliberate assignment paths retain ownership.
- [documented] `StatsView` and `DataState` already keep per-tab selection/scroll;
  `MenuModel::SetCategory` resets ITEMS selection/scroll. A transient inventory bookmark
  must therefore restore the previous row identity and bounded window after that
  existing route completes, without serializing a new save field.
- [hypothesis] Threshold comfort and gesture discoverability require physical user
  acceptance. CPU event-route tests do not establish device acceptance.
