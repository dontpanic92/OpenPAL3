import gen_rust
import parser
import sys

content = open(sys.argv[1], encoding="utf-8").read()
unit = parser.parse(content)
print(unit)

open("test.rs", "w").write(gen_rust.RustGen(unit, sys.argv[1]).gen())
