# Recovery editions

These five synthetic EPUBs preserve the real CLI/store recovery checks after removal of the legacy export oracle. They contain no private book text. All share the same publisher identifier; the test replaces the file at one canonical path to retain local book identity across editions.

The original Harbor chapter contains `The amber lantern rests.`. The other editions insert text before that passage, repeat it, replace it while retaining Harbor, or replace both passage and chapter. The short passage requires chapter confirmation after revision changes. `check_recovery.py` verifies exact unchanged-edition recovery and that every proposal leaves stored state byte-identical.
