import os
import shutil

os.system("python3 main.py idl/opengb.idl opengb::comdef")
shutil.copyfile("test.rs", "../../yaobow/opengb/src/comdef.rs")

os.system("python3 main.py idl/yaobow.idl yaobow::comdef")
shutil.copyfile("test.rs", "../../yaobow/yaobow/src/comdef.rs")

os.system("python3 main.py idl/yaobow_editor.idl yaobow_editor::comdef")
shutil.copyfile("test.rs", "../../yaobow/yaobow_editor/src/comdef.rs")

