import os
import shutil

os.system("python main.py idl/opengb_classes.idl opengb::classes")
shutil.copyfile("test.rs", "../../yaobow/opengb/src/classes.rs")
