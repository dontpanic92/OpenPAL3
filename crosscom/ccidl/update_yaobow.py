import os
import shutil

os.system("python main.py idl/yaobow.idl yaobow::comdef")
shutil.copyfile("test.rs", "../../yaobow/yaobow/src/comdef.rs")
