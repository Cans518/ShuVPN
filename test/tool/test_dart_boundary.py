import copy
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import verify_dart_boundary as boundary


class DartBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.write("pubspec.yaml", "name: shuvpn\ndependencies:\n  flutter:\n    sdk: flutter\n")
        self.write("pubspec.lock", "packages:\n  flutter:\n    dependency: direct main\n")
        self.write("flutter_rust_bridge.yaml", "web: false\n")
        self.write("rust/src/frb_generated.rs", "// generated bridge\n")
        self.write("lib/main.dart", "import 'view.dart';\nvoid main() {}\n")
        self.write("lib/view.dart", "class View {}\n")
        self.manifest = {
            "schema": 1, "generator": boundary.GENERATOR,
            "pubspec_sha256": boundary.text_hash(self.root / "pubspec.yaml"),
            "lockfile_sha256": boundary.text_hash(self.root / "pubspec.lock"),
            "runtime_dependencies": ["flutter"], "files": [],
            "generation_inputs": [
                {"path": path, "sha256": boundary.text_hash(self.root / path)}
                for path in ("rust/src/frb_generated.rs", "flutter_rust_bridge.yaml")
            ],
        }
        self.review()

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def review(self):
        self.manifest["files"] = [
            {"path": relative, "sha256": boundary.text_hash(path),
             "role": "view", "review": "Explicit test fixture review."}
            for relative, path in sorted(boundary.production_files(self.root).items())
        ]

    def test_matching_review_covers_every_production_file(self):
        result = boundary.verify(self.root, self.manifest)
        self.assertEqual(result["production_dart_files"], 2)

    def test_changed_implementation_requires_review_without_new_imports(self):
        self.write("lib/view.dart", "class View { int policy() => 7; }\n")
        with self.assertRaisesRegex(boundary.BoundaryError, "source changed"):
            boundary.verify(self.root, self.manifest)

    def test_added_generated_or_plugin_file_cannot_hide_from_inventory(self):
        for path in ("lib/core/rust/generated/extra.dart", "rust_builder/lib/plugin.dart"):
            with self.subTest(path=path):
                self.write(path, "// automatically generated\nclass Extra {}\n")
                with self.assertRaisesRegex(boundary.BoundaryError, "Unreviewed production"):
                    boundary.verify(self.root, self.manifest)
                (self.root / path).unlink()

    def test_backend_dependency_is_rejected_even_if_hash_was_updated(self):
        self.write("lib/view.dart", "import 'package:http/http.dart';\nclass View {}\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "backend dependency"):
            boundary.verify(self.root, self.manifest)

    def test_conditional_backend_import_is_also_checked(self):
        self.write("lib/main.dart",
                   "import 'view.dart' if (dart.library.io) 'package:dio/dio.dart';\n"
                   "void main() {}\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "backend dependency"):
            boundary.verify(self.root, self.manifest)

    def test_platform_io_needs_an_explicit_review(self):
        self.write("lib/view.dart", "import 'dart:io';\nclass View {}\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "platform I/O"):
            boundary.verify(self.root, self.manifest)
        self.manifest["files"][1]["io_review"] = "Only platform selection for this fixture."
        boundary.verify(self.root, self.manifest)

    def test_library_metadata_does_not_hide_platform_io(self):
        self.write("lib/view.dart",
                   "@Deprecated('legacy')\nlibrary;\nimport 'dart:io';\nclass View {}\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "platform I/O"):
            boundary.verify(self.root, self.manifest)

    def test_declaration_metadata_can_contain_parentheses_and_import_text(self):
        self.write("lib/view.dart",
                   "@Deprecated('old ) import text')\nclass View {}\n")
        self.review()
        boundary.verify(self.root, self.manifest)

    def test_unsupported_generic_header_metadata_fails_closed(self):
        self.write("lib/view.dart",
                   "@Marker<int>()\nlibrary;\nimport 'dart:io';\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "generic Dart header"):
            boundary.verify(self.root, self.manifest)

    def test_commented_js_condition_does_not_allow_an_unconditional_missing_stub(self):
        self.write("lib/core/rust/generated/frb_generated.dart",
                   "// automatically generated\n"
                   "// if (dart.library.js_interop) 'frb_generated.web.dart'\n"
                   "import 'frb_generated.web.dart';\n")
        self.review()
        for entry in self.manifest["files"]:
            if "/generated/" in entry["path"]:
                entry["role"] = "generated-frb"
        with self.assertRaisesRegex(boundary.BoundaryError, "leaves reviewed production"):
            boundary.verify(self.root, self.manifest)

    def test_comments_and_displayed_code_are_not_imports(self):
        self.write("lib/view.dart",
                   "/* outer /* import 'package:dio/dio.dart'; */ nested */\n"
                   "// import 'dart:io';\n"
                   "const shown = '''\nimport 'package:http/http.dart';\n''';\n")
        self.review()
        boundary.verify(self.root, self.manifest)

    def test_imports_cannot_reach_test_backends(self):
        self.write("test/backend.dart", "class Backend {}\n")
        self.write("lib/view.dart", "import '../test/backend.dart';\nclass View {}\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "leaves reviewed production"):
            boundary.verify(self.root, self.manifest)

    def test_dependency_or_generation_input_changes_invalidate_the_review(self):
        changed = copy.deepcopy(self.manifest)
        changed["generation_inputs"].pop()
        with self.assertRaisesRegex(boundary.BoundaryError, "input coverage"):
            boundary.verify(self.root, changed)
        self.write("pubspec.yaml", "name: shuvpn\ndependencies:\n  http: any\n")
        with self.assertRaisesRegex(boundary.BoundaryError, "pubspec.yaml changed"):
            boundary.verify(self.root, self.manifest)

    def test_transitive_lockfile_changes_also_require_review(self):
        self.write("pubspec.lock", "packages:\n  changed_transitive:\n    version: 99\n")
        with self.assertRaisesRegex(boundary.BoundaryError, "pubspec.lock changed"):
            boundary.verify(self.root, self.manifest)

    def test_native_only_generated_stub_does_not_allow_other_missing_files(self):
        path = "lib/core/rust/generated/frb_generated.dart"
        self.write(path, "// This file is automatically generated.\n"
                   "import 'frb_generated.io.dart'\n"
                   " if (dart.library.js_interop) 'frb_generated.web.dart';\nclass Bridge {}\n")
        self.write("lib/core/rust/generated/frb_generated.io.dart",
                   "// This file is automatically generated.\nclass NativeBridge {}\n")
        self.review()
        for entry in self.manifest["files"]:
            if "/generated/" in entry["path"]:
                entry["role"] = "generated-frb"
        boundary.verify(self.root, self.manifest)
        self.write(path, "// This file is automatically generated.\nimport 'missing.dart';\n")
        self.review()
        with self.assertRaisesRegex(boundary.BoundaryError, "leaves reviewed production"):
            boundary.verify(self.root, self.manifest)
