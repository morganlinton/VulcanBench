import os
import re
import glob

TEST_DIR = "/media/mukund-thiru/SanthData/Santh/libs/performance/matching/vyre/vyre-driver-wgpu/tests"

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    pattern = re.compile(
        r'(?:#\[derive\(Clone,\s*Copy\)\]\n)?(?:pub\(crate\)\s+)?struct FixtureToken\s*\{.*?'
        r'(?:impl FixtureToken\s*\{.*?\}\n)?\n*'
        r'(?:#\[derive\(Clone\)\]\n)?(?:pub\(crate\)\s+)?struct Fixture\s*\{.*?\n\}\n*'
        r'(?:pub\(crate\)\s+)?fn build_fixture\s*\([^)]*\)\s*->\s*Fixture\s*\{.*?\n\}\n',
        re.DOTALL
    )

    if not pattern.search(content):
        return

    print(f"Modifying {filepath}")
    
    replacement = "\nmod common;\nuse common::c_fixture::*;\n"
    if "mod common;" in content:
        replacement = "\nuse common::c_fixture::*;\n"
        
    new_content = pattern.sub(replacement, content, count=1)
    
    with open(filepath, 'w') as f:
        f.write(new_content)

files = glob.glob(os.path.join(TEST_DIR, "*.rs"))
files.extend(glob.glob(os.path.join(TEST_DIR, "*", "mod.rs")))
for f in files:
    process_file(f)
