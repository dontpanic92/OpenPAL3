import os
import shutil

os.system("python3 main.py idl/openpal3.idl shared::openpal3::comdef")
shutil.copyfile("test.rs", "../../yaobow/shared/src/openpal3/comdef.rs")

os.system("python3 main.py idl/openpal4.idl shared::openpal4::comdef")
shutil.copyfile("test.rs", "../../yaobow/shared/src/openpal4/comdef.rs")

os.system("python3 main.py idl/openpal5.idl shared::openpal5::comdef")
shutil.copyfile("test.rs", "../../yaobow/shared/src/openpal5/comdef.rs")

os.system("python3 main.py idl/openswd5.idl shared::openswd5::comdef")
shutil.copyfile("test.rs", "../../yaobow/shared/src/openswd5/comdef.rs")

os.system("python3 main.py idl/yaobow.idl yaobow::comdef")
shutil.copyfile("test.rs", "../../yaobow/yaobow/src/comdef.rs")

os.system("python3 main.py idl/yaobow_editor.idl yaobow_editor::comdef")
shutil.copyfile("test.rs", "../../yaobow/yaobow_editor/src/comdef.rs")

