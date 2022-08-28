import os
import shutil

os.system("python main.py idl/radiance.idl radiance::interfaces")
shutil.copyfile("test.rs", "../../radiance/src/interfaces.rs")
