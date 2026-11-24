# How fast does Rad Child give Health back at each stage of radiation sickness?

Priority: P1
Needs: the observer, unattended; no particular save (outside Hardcore)
Spec: #3471, [aid-items.md](../../spec-notes/aid-items.md) §3

The four stage spells carry "Regeneration" of magnitude 2, 4, 6 and 8 behind
`HasPerk RadChild`; no record gives the unit. Ours [hypothesis]: magnitude Health a second
(`fnv_game::aid::REGENERATION_RATE` 1), evenly, from at most 1 s after the perk, stopping at
the maximum.

- Set-up: outside Hardcore, indoors, no fight, god mode off, rads not rising, no Solar
  Powered, implant, food or chem effect. `ModActorValue Health 1000`, then
  `DamageActorValue Health 900`, so that no run reaches the maximum; log the maximum
  too, as the stages' Endurance penalties may move it.
- For rads at 250, 450, 650 and 850 (`SetActorValue RadiationRads`), without the perk
  at the start: wait 2 s and log `GetAV Health` every frame for 10 s (it should not
  move); `player.AddPerk RadChild` and log `GetAV Health`, `GetAV RadiationRads`,
  `IsSpellTarget` of the stage's spell and the seconds passed for **60 s**;
  `player.RemovePerk RadChild` and log 10 s more.
- Report per stage: the rise over the 60 s, the size and spacing of the steps (every
  frame, every second, or longer), the delay after AddPerk and after RemovePerk.
- Then with the perk, still logging: rads 450 to 650 and back (does the rate swap at
  once, is there a jump at the swap); rads 199 (does it stop); Health close under the
  maximum at one stage (does it stop there); one hour waited at rads 250 with Health
  damaged by 900 (Health before and after).
- If cheap, the same base effect elsewhere: Solar Powered (magnitude 1) outdoors at
  noon for 60 s and indoors for 10 s.
- If cheap, on other actors (the engine's change makes them regenerate too): the
  Legate (`RegenMed`, magnitude 5, given by `VLegateScript`) and one Dead Money
  hologram (`NVDLC01HologramRegen`, 100). Out of combat, log `IsSpellTarget` of the
  spell (AddSpell it if 0, and say so), `DamageActorValue Health` by half of its
  Health, then log `GetAV Health` every frame for **30 s**. Report the rise, the
  steps and where it stops.
