import os
import shutil

os.system("python3 main.py idl/opengb.idl opengb::comdef")
shutil.copyfile("test.rs", "../../yaobow/opengb/src/comdef.rs")
