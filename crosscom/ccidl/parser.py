from dataclasses import dataclass
from parsy import forward_declaration, regex, seq, string, whitespace, letter, any_char, digit


@dataclass
class MethodParameter:
    attrs: list[str]
    name: str
    ty: str


@dataclass
class Method:
    name: str
    ret_ty: str
    params: list[MethodParameter]
    attrs: map

@dataclass
class Import:
    file_name: str

@dataclass
class Module:
    module_lang: str
    module_name: str

@dataclass
class Interface:
    name: str
    bases: list[str]
    methods: list[Method]
    attrs: map

    __public_methods: list[Method] = None
    __internal_methods: list[Method] = None

    def __init__(self, name, bases, methods, attrs):
        self.name = name
        self.bases = bases
        self.methods = methods
        self.attrs = attrs

        for method in self.methods:
            method.interface = self

    def codegen_ignore(self):
        return self.attrs is not None and 'codegen' in self.attrs and self.attrs['codegen'] == 'ignore'

    def public_methods(self):
        if self.__public_methods is None:
            self.__public_methods = []
            for m in self.methods:
                if m.attrs is None or 'internal' not in m.attrs:
                    self.__public_methods.append(m)

        return self.__public_methods

    def internal_methods(self):
        if self.__internal_methods is None:
            self.__internal_methods = []
            for m in self.methods:
                if  m.attrs is not None and 'internal' in m.attrs:
                    self.__internal_methods.append(m)

        return self.__internal_methods


@dataclass
class Class:
    name: str
    bases: list[str]
    methods: list[Method]
    attrs: map


@dataclass
class CrossComIdl:
    items: list[Class | Interface]
    imports: list[Import]
    modules: list[Module]

    def find(self, name: str) -> None | Class | Interface:
        for i in self.items:
            if not isinstance(i, Import) and i.name == name:
                return i

        return None


padding = whitespace.optional()
def lexeme(p): return p << padding


lbrace = lexeme(string("{"))
rbrace = lexeme(string("}"))
lbrack = lexeme(string("["))
rbrack = lexeme(string("]"))
lparen = lexeme(string("("))
rparen = lexeme(string(")"))
colon = lexeme(string(":"))
comma = lexeme(string(","))
semicolon = lexeme(string(";"))

skip = lparen | rparen | letter | digit | string(
    "-") | string(",") | whitespace | semicolon


def test2(*args, **kwargs):
    print(args)
    print(kwargs)
    return args


identifier = (string('dyn ') | letter | digit | string('&mut ') | regex(r'[_&]') | string('::') | string('?') | regex(r'[<>]') | string(".")).at_least(1).map("".join)
ty = (identifier + string('[]')) | (identifier + string('*')) | identifier

attr_value = regex(r"[^()]").many().map("".join)
attributes = (lbrack >> (
    seq(identifier << lparen, attr_value << rparen)).sep_by(comma) << rbrack).map(lambda p: {i[0]: i[1] for i in p})

method_param = seq(
    attrs=(lbrack >> identifier.sep_by(comma) << rbrack).optional(),
    ty=identifier << whitespace,
    name=identifier << padding,
).combine_dict(MethodParameter)

method = seq(
    attrs=(attributes << padding).optional(),
    ret_ty=ty << whitespace,
    name=identifier << padding,
    params=lparen >> method_param.sep_by(comma) << rparen << semicolon,
).combine_dict(Method)


def def_top_level(keyword: str, ty: type):
    return seq(
        attrs=attributes << padding,
        _1=string(keyword) << whitespace,
        name=identifier << padding,
        bases=(colon >> identifier.sep_by(comma) << padding).optional(),
        methods=lbrace >> method.many() << rbrace,
    ).combine_dict(ty)


interface = def_top_level("interface", Interface)
klass = def_top_level("class", Class)
imqort = seq(
    file_name=string("import") >> padding >> identifier << padding << semicolon,
).combine_dict(Import)
module = seq(
    module_lang=string("module") >> padding >> lparen >> identifier << rparen,
    module_name=identifier << padding << semicolon,
).combine_dict(Module)

def collect_unit(top_level_list):
    items = []
    imports = []
    modules = []
    for item in top_level_list:
        if isinstance(item, Import):
            imports.append(item)
        elif isinstance(item, Module):
            modules.append(item)
        else:
            items.append(item)
    
    return CrossComIdl(items, imports, modules)

top_level = interface | klass | imqort | module
unit = top_level.many().map(collect_unit)


def parse(content: str) -> CrossComIdl:
    return unit.parse(content)
