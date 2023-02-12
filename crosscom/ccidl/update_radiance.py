import os
import shutil

os.system("python3 main.py idl/radiance.idl radiance::comdef")
shutil.copyfile("test.rs", "../../radiance/radiance/src/comdef.rs")
