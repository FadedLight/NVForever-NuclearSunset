# Existing menus and independent sizes

Buying and selling, repair, and crafting keep their ordinary menu rendering and model actions. Their size settings are independent: `barter-menu-size-v1`, `repair-menu-size-v1`, and `workbench-menu-size-v1`. Each stores a slider part for 50–150% in 5% steps; missing values use 100%. Menu renderers must apply the selected size to actual text, rows, clipping and hit geometry together. A resource or saved slider alone does not establish that integration.

The expanded global HUD slider uses `hud-scale-v2`, spanning 25–250% in 5% steps. Its value remains a normalized slider part. Existing `ui-scale` values retain their old 50–150% interpretation during migration, rather than being reinterpreted in the wider range. An explicit new key wins. Independent `menu-scale` values keep their old units and precedence. The HUD size still takes effect at the next start; separate menu sizes are consumed by their owning renderers.

The existing settings writer owns persistence. The new source controls cover each endpoint, independent menu values, the actual INI write/reload path and legacy HUD migration. The original narrow-range codec and its tests remain as compatibility controls. Placement tables retain every earlier key and add the new rows on desktop and Android, including the dialogue owner's background-tint option.

This source preparation is uncompiled and unrun. Menu rendering/drag integration is authored separately through the existing consumers and must be assembled and exercised before claiming working menu sizing. No physical-device, screenshots, touch acceptance or crash correction is established by these fixtures.
