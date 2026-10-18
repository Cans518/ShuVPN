from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import desktop_tray_acceptance as tray


class TrayOwnershipTest(unittest.TestCase):
    def test_selects_only_the_owned_process_and_preserves_exported_path(self):
        pids = {":1.2": 700, ":1.9": 900}
        self.assertEqual(
            tray.owned_item([":1.2/Other", ":1.9/owned/Menu"], 900, pids.__getitem__),
            (":1.9", "/owned/Menu"),
        )

    def test_missing_or_ambiguous_owned_items_are_not_accepted(self):
        with self.assertRaises(LookupError):
            tray.owned_item([":1.2/Other"], 900, lambda _: 700)
        with self.assertRaises(RuntimeError):
            tray.owned_item([":1.9/One", ":1.9/Two"], 900, lambda _: 900)
        with self.assertRaises(RuntimeError):
            tray.owned_item([":1.9"], 900, lambda _: 900)

    def test_nested_menu_keeps_native_ids_and_disabled_state(self):
        rows = tray.menu_rows((0, {}, [
            (11, {"label": "显示窗口"}, []),
            (40, {"label": "协议"}, [
                (81, {"label": "OpenVPN", "enabled": False}, []),
            ]),
        ]))
        self.assertEqual(rows[-1], {"id": 81, "label": "OpenVPN", "enabled": False})
        self.assertEqual([row["id"] for row in rows], [0, 11, 40, 81])

    def test_watcher_must_descend_from_the_owned_panel(self):
        self.assertEqual(tray.descendant_chain(12, 10, {12: 11, 11: 10}.__getitem__),
                         [12, 11, 10])
        for parents in ({12: 1, 1: 0}, {12: 11, 11: 12}):
            with self.subTest(parents=parents), self.assertRaises(RuntimeError):
                tray.descendant_chain(12, 10, parents.__getitem__)
