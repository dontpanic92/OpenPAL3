from dp_redo import *
import os
import os, sys, re

source_tree = os.path.dirname(os.path.abspath(sys.argv[0]))
shader_target_folder = os.path.join(source_tree, "target/resources/shaders")
shader_source_folder = os.path.join(source_tree, "src/shaders")

def cargo_build():
    cmd("cargo build")

@do(".spv")
def glslc(target_name, target_base_name, output_file):
    redo_ifchange(shader_source(target_base_name))
    cmd("glslc {} -o {}".format(shader_source(target_base_name), output_file))

@do("__debug")
def build(t, b, o):
    cargo_build()
    make_dirs()
    redo_ifchange(
        shader_target("simple_triangle.frag.spv"),
        shader_target("simple_triangle.vert.spv"))

#### Helper functions ####
def cmd(command):
    print("Calling {}".format(command))
    code = os.system(command)
    if code != 0:
        print("Error calling {}: returning {}".format(command, code))
        exit(1)

def shader_target(shader_name):
    return os.path.join(shader_target_folder, shader_name)

def shader_source(shader_name):
    return os.path.join(shader_source_folder, shader_name)

def make_dirs():
    dirs = [shader_target_folder]
    for d in dirs:
        if not os.path.exists(d):
            os.makedirs(d)

if __name__ == "__main__":
    redo_ifchange(build)
    sys.exit(0)