import os
import shutil

os.system("python3 main.py idl/editor.idl radiance_editor::comdef")
shutil.copyfile("test.rs", "../../radiance/radiance_editor/src/comdef.rs")
