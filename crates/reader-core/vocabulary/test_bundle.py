import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from dictionary import build, bundled_dictionary, lookup


class BundleTests(unittest.TestCase):
    def fixture(self, root):
        source = root / 'source.jsonl'
        source.write_text(json.dumps({'word': 'whorl', 'lang_code': 'en', 'pos': 'noun',
                                      'senses': [{'glosses': ['A spiral.']} ]}) + '\n')
        database = root / 'original.sqlite'
        build(source, database)
        data = database.read_bytes()
        resources = root / 'resources'
        resources.mkdir()
        (resources / 'english.sqlite.gz').write_bytes(gzip.compress(data, mtime=0))
        (resources / 'manifest.json').write_text(json.dumps({
            'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}))
        return resources, data

    def test_shipped_definitions_expand_once_and_remain_read_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resources, data = self.fixture(root)
            path = bundled_dictionary(root / 'cache', resources)
            self.assertEqual(path.read_bytes(), data)
            self.assertEqual(lookup(path, 'whorl')['entries'][0]['senses'][0]['glosses'], ['A spiral.'])
            self.assertEqual(path.read_bytes(), data)
            (resources / 'english.sqlite.gz').unlink()
            self.assertEqual(bundled_dictionary(root / 'cache', resources), path)

    def test_corrupt_bundle_never_publishes_a_partial_database(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resources, data = self.fixture(root)
            (resources / 'english.sqlite.gz').write_bytes(gzip.compress(data[:-1], mtime=0))
            with self.assertRaisesRegex(ValueError, 'checksum'):
                bundled_dictionary(root / 'cache', resources)
            self.assertEqual(list((root / 'cache').iterdir()), [])
