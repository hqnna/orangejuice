//
// Program_Print: a `Code_Node` tree written back out as source (**C§5.3**).
//
// `print_expression(*builder, node)` writes what a node says the way a person
// would have written it — not byte for byte what was in the file, since the
// tree does not keep spacing or comments, but text that parses back to the
// same tree. It is what a metaprogram uses to show the code it was handed or
// the code it built.
//
// As a plugin (`-plug Program_Print`) it writes every top-level declaration a
// compilation typechecked in one of the program's own files to
// `pp_<file>.txt` beside that file.
//

#import "Basic";
#import "Compiler";
#import "String";
#import "File";

print_expression :: (builder: *String_Builder, node: *Code_Node) {
    printer: Printer;
    printer.builder = builder;
    expression(*printer, node);
}

// The same, into a string in temporary storage.
print_expression_to_string :: (node: *Code_Node) -> string {
    builder: String_Builder;
    builder.allocator = temp;
    print_expression(*builder, node);
    return builder_to_string(*builder,, temp);
}

// ------------------------------------------------------------- the plugin ---

Program_Print_Plugin :: struct {
    #as using base: Metaprogram_Plugin;
    by_file: [..] Printed_File;
}

Printed_File :: struct {
    path:    string;
    builder: String_Builder;
}

get_plugin :: () -> *Metaprogram_Plugin {
    plugin := New(Program_Print_Plugin);
    plugin.message = plugin_message;
    plugin.finish = plugin_finish;
    plugin.shutdown = plugin_shutdown;
    return plugin;
}

#scope_file

plugin_message :: (p: *Metaprogram_Plugin, message: *Message) {
    plugin := cast(*Program_Print_Plugin) p;
    if message.kind != .TYPECHECKED  return;
    typechecked := cast(*Message_Typechecked) message;
    for typechecked.declarations {
        declaration := it.expression;
        if !declaration || !declaration.enclosing_load  continue;
        // Only the program's own files: the modules it imports are not what
        // anyone asked to see, and their directories are not the program's.
        load := declaration.enclosing_load;
        if load.enclosing_import && load.enclosing_import.module_type != .MAIN_PROGRAM  continue;
        path := load.fully_pathed_filename;
        file: *Printed_File;
        for * plugin.by_file  if it.path == path { file = it; break; }
        if !file {
            file = array_add(*plugin.by_file);
            file.path = path;
        }
        print_expression(*file.builder, declaration);
        append(*file.builder, "\n");
    }
}

plugin_finish :: (p: *Metaprogram_Plugin) {
    plugin := cast(*Program_Print_Plugin) p;
    for * plugin.by_file {
        directory, name := path_and_name(it.path);
        write_entire_file(tprint("%pp_%.txt", directory, name), *it.builder);
    }
}

plugin_shutdown :: (p: *Metaprogram_Plugin) {
    free(p);
}

path_and_name :: (path: string) -> (directory: string, name: string) {
    slash := -1;
    for 0..path.count - 1  if path[it] == #char "/"  slash = it;
    if slash < 0  return "", path;
    directory := path;
    directory.count = slash + 1;
    name := path;
    name.data += slash + 1;
    name.count -= slash + 1;
    if name.count > 4 && slice(name, name.count - 4, 4) == ".jai"  name.count -= 4;
    return directory, name;
}

// ------------------------------------------------------------ the printer ---

Printer :: struct {
    builder: *String_Builder;
    indent:  s64;
}

write :: (p: *Printer, text: string) {
    append(p.builder, text);
}

newline :: (p: *Printer) {
    append(p.builder, "\n");
    for 1..p.indent  append(p.builder, "    ");
}

operator_text :: (op: Operator_Type) -> string {
    if op == {
        case .IS_EQUAL;            return "==";
        case .IS_NOT_EQUAL;        return "!=";
        case .LOGICAL_AND;         return "&&";
        case .LOGICAL_OR;          return "||";
        case .LESS_OR_EQUAL;       return "<=";
        case .GREATER_OR_EQUAL;    return ">=";
        case .SHIFT_LEFT;          return "<<";
        case .SHIFT_RIGHT;         return ">>";
        case .ROTATE_LEFT;         return "<<<";
        case .ROTATE_RIGHT;        return ">>>";
        case .PLUS_ASSIGN;         return "+=";
        case .MINUS_ASSIGN;        return "-=";
        case .TIMES_ASSIGN;        return "*=";
        case .DIV_ASSIGN;          return "/=";
        case .MOD_ASSIGN;          return "%=";
        case .SHIFT_LEFT_ASSIGN;   return "<<=";
        case .SHIFT_RIGHT_ASSIGN;  return ">>=";
        case .ROTATE_LEFT_ASSIGN;  return "<<<=";
        case .ROTATE_RIGHT_ASSIGN; return ">>>=";
        case .BITWISE_AND_ASSIGN;  return "&=";
        case .BITWISE_OR_ASSIGN;   return "|=";
        case .BITWISE_XOR_ASSIGN;  return "^=";
        case .LOGICAL_AND_ASSIGN;  return "&&=";
        case .LOGICAL_OR_ASSIGN;   return "||=";
        case .POINTER_DEREFERENCE; return "<<";
        case .POSTFIX_DEREFERENCE; return ".*";
    }
    // A single-character operator is its own character.
    character := cast(u8) op;
    text: string;
    text.data = talloc(1);
    text.count = 1;
    text[0] = character;
    return text;
}

string_literal :: (p: *Printer, text: string) {
    write(p, "\"");
    for 0..text.count - 1 {
        c := text[it];
        if c == {
            case #char "\"";  write(p, "\\\"");
            case #char "\\";  write(p, "\\\\");
            case #char "\n";  write(p, "\\n");
            case #char "\t";  write(p, "\\t");
            case #char "\r";  write(p, "\\r");
            case 0;           write(p, "\\0");
            case;
                if c < 32 || c == 127  print_to_builder(p.builder, "\\x%", formatInt(c, base = 16, minimum_digits = 2));
                else {
                    single: string;
                    single.data = *c;
                    single.count = 1;
                    write(p, single);
                }
        }
    }
    write(p, "\"");
}

literal :: (p: *Printer, node: *Code_Literal) {
    if node.value_type == {
        case .NUMBER;
            if node.value_flags & .FLOAT        print_to_builder(p.builder, "%", node._float64);
            else if node.value_flags & .HEX     print_to_builder(p.builder, "0x%", formatInt(node._u64, base = 16));
            else if node.value_flags & .BINARY  print_to_builder(p.builder, "0b%", formatInt(node._u64, base = 2));
            else                                print_to_builder(p.builder, "%", node._u64);
        case .STRING;
            string_literal(p, node._string);
        case .BOOLEAN;
            write(p, ifx node._s64 then "true" else "false");
        case .POINTER;
            info := node.pointer_literal_info;
            if !info || info.pointer_literal_type == .CONSTANT_VALUE  write(p, "null");
            else if info.global_symbol  write(p, info.global_symbol.name);
            else write(p, "null");
        case .STRUCT;
            info := node.struct_literal_info;
            if info && info.type_expression  expression(p, info.type_expression);
            write(p, ".{");
            if info  list(p, info.arguments);
            write(p, "}");
        case .ARRAY;
            info := node.array_literal_info;
            if info && info.element_type  expression(p, info.element_type);
            write(p, ".[");
            if info  list(p, info.array_members);
            write(p, "]");
        case .TYPE_INFO;
            defn := node.type_info_literal_defn;
            if defn && defn.info  print_type_to_builder(p.builder, defn.info);
        case;
            write(p, "/* literal */");
    }
}

list :: (p: *Printer, nodes: [] *$T) {
    for nodes {
        if it_index  write(p, ", ");
        expression(p, it);
    }
}

arguments :: (p: *Printer, arguments: [] Code_Argument) {
    for arguments {
        if it_index  write(p, ", ");
        if it.name {
            write(p, it.name.name);
            write(p, " = ");
        }
        expression(p, it.expression);
    }
}

type_instantiation :: (p: *Printer, inst: *Code_Type_Instantiation) {
    if inst.result && !inst.type_valued_expression && !inst.pointer_to && !inst.array_element_type {
        print_type_to_builder(p.builder, inst.result);
        return;
    }
    if inst.inst_flags & .VARARGS  write(p, "..");
    if inst.inst_flags & .TYPE_DIRECTIVE {
        write(p, "#type");
        if inst.inst_flags & .TYPE_DIRECTIVE_DISTINCT  write(p, ",distinct");
        if inst.inst_flags & .TYPE_DIRECTIVE_ISA       write(p, ",isa");
        write(p, " ");
        if inst.type_directive_target  expression(p, inst.type_directive_target);
        return;
    }
    if inst.pointer_to {
        write(p, "*");
        expression(p, inst.pointer_to);
        return;
    }
    if inst.array_element_type {
        write(p, "[");
        if inst.inst_flags & .RESIZABLE    write(p, "..");
        else if inst.array_dimension       expression(p, inst.array_dimension);
        write(p, "] ");
        expression(p, inst.array_element_type);
        return;
    }
    if inst.type_valued_expression  expression(p, inst.type_valued_expression);
    if inst.must_implement {
        write(p, "/");
        expression(p, inst.must_implement);
    }
}

declaration :: (p: *Printer, node: *Code_Declaration) {
    if node.flags & .IS_MARKED_AS_AS  write(p, "#as ");
    if node.flags & .NO_RESET         write(p, "#no_reset ");
    write(p, node.name);
    constant := (node.flags & .IS_CONSTANT) != 0;
    if node.type_inst {
        write(p, ": ");
        expression(p, node.type_inst);
        if node.flags & .IS_UNINITIALIZED {
            write(p, " = ---");
        } else if node.expression {
            write(p, ifx constant then " : " else " = ");
            expression(p, node.expression);
        }
    } else {
        write(p, ifx constant then " :: " else " := ");
        if node.flags & .IS_UNINITIALIZED  write(p, "---");
        else if node.expression            expression(p, node.expression);
    }
    for node.notes {
        write(p, " @");
        write(p, it.text);
    }
}

block :: (p: *Printer, node: *Code_Block) {
    write(p, "{");
    p.indent += 1;
    for node.statements {
        newline(p);
        statement(p, it);
    }
    p.indent -= 1;
    newline(p);
    write(p, "}");
}

needs_semicolon :: (node: *Code_Node) -> bool {
    if node.kind == {
        case .BLOCK; #through;
        case .IF; #through;
        case .WHILE; #through;
        case .FOR; #through;
        case .CASE; #through;
        case .STRUCT; #through;
        case .ENUM;
            return false;
        case .DECLARATION;
            value := (cast(*Code_Declaration) node).expression;
            if value && (value.kind == .PROCEDURE_HEADER || value.kind == .STRUCT || value.kind == .ENUM)  return false;
    }
    return true;
}

statement :: (p: *Printer, node: *Code_Node) {
    if !node  return;
    expression(p, node);
    if needs_semicolon(node)  write(p, ";");
}

procedure_header :: (p: *Printer, node: *Code_Procedure_Header) {
    write(p, "(");
    for node.arguments {
        if it_index  write(p, ", ");
        // A `using` parameter is exported as the `using` around it.
        if it.kind == .DECLARATION  declaration(p, it);
        else expression(p, it);
    }
    write(p, ")");
    if node.returns.count {
        write(p, " -> ");
        several := node.returns.count > 1;
        if several  write(p, "(");
        for node.returns {
            if it_index  write(p, ", ");
            if it.kind != .DECLARATION {
                expression(p, it);
                continue;
            }
            if it.name.count {
                write(p, it.name);
                write(p, ": ");
            }
            if it.type_inst  expression(p, it.type_inst);
            else if it.expression  expression(p, it.expression);
        }
        if several  write(p, ")");
    }
    if node.procedure_flags & .MACRO              write(p, " #expand");
    if node.procedure_flags & .C_CALL             write(p, " #c_call");
    if node.procedure_flags & .COMPILE_TIME_ONLY  write(p, " #compile_time");
    if node.procedure_flags & .SYMMETRIC          write(p, " #symmetric");
    if node.procedure_flags & .ELSEWHERE {
        write(p, " #foreign");
        if node.library_identifier {
            write(p, " ");
            write(p, node.library_identifier.name);
        }
        if node.foreign_function_name.count && node.foreign_function_name != node.name {
            write(p, " ");
            string_literal(p, node.foreign_function_name);
        }
    }
    if node.body_or_null && node.body_or_null.block {
        write(p, " ");
        block(p, node.body_or_null.block);
    }
}

expression :: (p: *Printer, node: *Code_Node) {
    if !node  return;
    parenthesized := (node.node_flags & .IS_PARENTHESIZED) != 0 && node.kind != .BLOCK;
    if parenthesized  write(p, "(");
    defer if parenthesized  write(p, ")");

    if node.kind == {
        case .LITERAL;
            literal(p, cast(*Code_Literal) node);

        case .IDENT;
            ident := cast(*Code_Ident) node;
            if ident.flags & .HAS_SCOPE_MODIFIER  write(p, "`");
            write(p, ident.name);

        case .UNARY_OPERATOR;
            unary := cast(*Code_Unary_Operator) node;
            if unary.operator_type == .POSTFIX_DEREFERENCE {
                expression(p, unary.subexpression);
                write(p, ".*");
            } else {
                write(p, operator_text(unary.operator_type));
                expression(p, unary.subexpression);
            }

        case .BINARY_OPERATOR;
            binary := cast(*Code_Binary_Operator) node;
            if binary.operator_type == .ARRAY_SUBSCRIPT {
                expression(p, binary.left);
                write(p, "[");
                expression(p, binary.right);
                write(p, "]");
            } else if cast(s32) binary.operator_type == #char "." {
                expression(p, binary.left);
                write(p, ".");
                expression(p, binary.right);
            } else {
                expression(p, binary.left);
                write(p, " ");
                write(p, operator_text(binary.operator_type));
                write(p, " ");
                expression(p, binary.right);
            }

        case .PROCEDURE_CALL;
            call := cast(*Code_Procedure_Call) node;
            if call.flags & .INLINE_YES  write(p, "inline ");
            if call.flags & .INLINE_NO   write(p, "no_inline ");
            expression(p, call.procedure_expression);
            write(p, "(");
            arguments(p, call.arguments_unsorted);
            if call.context_modification {
                write(p, ",, ");
                list(p, call.context_modification.modification_expressions);
            }
            write(p, ")");

        case .CONTEXT;
            write(p, "context");

        case .BLOCK;
            block(p, cast(*Code_Block) node);

        case .WHILE;
            loop := cast(*Code_While) node;
            write(p, "while ");
            expression(p, loop.condition);
            write(p, " ");
            statement(p, loop.block);

        case .IF;
            branch := cast(*Code_If) node;
            if branch.if_flags & .IS_IFX {
                write(p, "ifx ");
                expression(p, branch.condition);
                write(p, " then ");
                expression(p, branch.then_block);
                if branch.else_block {
                    write(p, " else ");
                    expression(p, branch.else_block);
                }
            } else {
                write(p, ifx branch.if_flags & .IS_STATIC then "#if " else "if ");
                expression(p, branch.condition);
                if branch.if_flags & .IS_SWITCH_STATEMENT {
                    write(p, " == ");
                    expression(p, branch.then_block);
                } else {
                    write(p, " ");
                    statement(p, branch.then_block);
                    if branch.else_block {
                        write(p, " else ");
                        statement(p, branch.else_block);
                    }
                }
            }

        case .CASE;
            case_node := cast(*Code_Case) node;
            write(p, "case");
            if case_node.condition {
                write(p, " ");
                expression(p, case_node.condition);
            }
            write(p, ";");
            if case_node.marked_as_fallthrough  write(p, " #through;");
            if case_node.then_block {
                p.indent += 1;
                for case_node.then_block.statements {
                    newline(p);
                    statement(p, it);
                }
                p.indent -= 1;
            }

        case .LOOP_CONTROL;
            control := cast(*Code_Loop_Control) node;
            if control.control_type == {
                case .BREAK;    write(p, "break");
                case .CONTINUE; write(p, "continue");
                case .REMOVE;   write(p, "remove");
            }
            if control.target_ident {
                write(p, " ");
                write(p, control.target_ident.name);
            }

        case .RETURN;
            ret := cast(*Code_Return) node;
            if ret.return_flags & .IS_BACKTICKED  write(p, "`");
            write(p, "return");
            if ret.arguments_unsorted.count {
                write(p, " ");
                arguments(p, ret.arguments_unsorted);
            }

        case .FOR;
            loop := cast(*Code_For) node;
            write(p, "for ");
            if loop.for_flags & .REVERSE  write(p, "< ");
            if loop.for_flags & .POINTER  write(p, "* ");
            if loop.ident_it && loop.ident_it.name != "it" {
                write(p, loop.ident_it.name);
                if loop.ident_it_index && loop.ident_it_index.name != "it_index" {
                    write(p, ", ");
                    write(p, loop.ident_it_index.name);
                }
                write(p, ": ");
            }
            expression(p, loop.iteration_expression);
            if loop.iteration_expression_right {
                write(p, "..");
                expression(p, loop.iteration_expression_right);
            }
            write(p, " ");
            statement(p, loop.block);

        case .TYPE_INSTANTIATION;
            type_instantiation(p, cast(*Code_Type_Instantiation) node);

        case .TYPE_DEFINITION;
            defn := cast(*Code_Type_Definition) node;
            if defn.info  print_type_to_builder(p.builder, defn.info);

        case .ENUM;
            enumeration := cast(*Code_Enum) node;
            write(p, ifx enumeration.is_flags then "enum_flags" else "enum");
            if enumeration.internal_type_inst {
                write(p, " ");
                expression(p, enumeration.internal_type_inst);
            }
            if enumeration.marked_as_specified  write(p, " #specified");
            if enumeration.marked_as_complete   write(p, " #complete");
            write(p, " ");
            if enumeration.block  block(p, enumeration.block);

        case .PROCEDURE_HEADER;
            procedure_header(p, cast(*Code_Procedure_Header) node);

        case .PROCEDURE_BODY;
            body := cast(*Code_Procedure_Body) node;
            if body.header  procedure_header(p, body.header);
            else if body.block  block(p, body.block);

        case .STRUCT;
            structure := cast(*Code_Struct) node;
            write(p, ifx structure.textual_flags & .UNION then "union" else "struct");
            if structure.textual_flags & .NO_PADDING  write(p, " #no_padding");
            write(p, " ");
            if structure.block  block(p, structure.block);

        case .COMMA_SEPARATED_ARGUMENTS;
            comma := cast(*Code_Comma_Separated_Arguments) node;
            for comma.arguments {
                if it_index  write(p, ", ");
                expression(p, it.node);
            }

        case .EXTRACT;
            expression(p, (cast(*Code_Extract) node).from);

        case .MAKE_VARARGS;
            list(p, (cast(*Code_Make_Varargs) node).expressions);

        case .DECLARATION;
            declaration(p, cast(*Code_Declaration) node);

        case .COMPOUND_DECLARATION;
            compound := cast(*Code_Compound_Declaration) node;
            if compound.comma_separated_assignment  expression(p, compound.comma_separated_assignment);
            properties := compound.declaration_properties;
            if properties {
                constant := (properties.flags & .IS_CONSTANT) != 0;
                if properties.type_inst {
                    write(p, ": ");
                    expression(p, properties.type_inst);
                    if properties.expression {
                        write(p, ifx constant then " : " else " = ");
                        expression(p, properties.expression);
                    }
                } else if properties.expression {
                    write(p, ifx constant then " :: " else " := ");
                    expression(p, properties.expression);
                }
            }

        case .CAST;
            cast_node := cast(*Code_Cast) node;
            if cast_node.cast_flags & .IS_AUTO {
                write(p, "xx ");
                expression(p, cast_node.expression);
            } else {
                write(p, "cast");
                if cast_node.cast_flags & .TRUNCATE         write(p, ",trunc");
                if cast_node.cast_flags & .NO_BOUNDS_CHECK  write(p, ",no_check");
                if cast_node.cast_flags & .FORCE            write(p, ",force");
                write(p, "(");
                expression(p, cast_node.target_type);
                write(p, ") ");
                expression(p, cast_node.expression);
            }

        case .DIRECTIVE_IMPORT;
            import := cast(*Code_Directive_Import) node;
            write(p, "#import ");
            string_literal(p, import.name);

        case .DIRECTIVE_LOAD;
            load := cast(*Code_Directive_Load) node;
            write(p, "#load ");
            string_literal(p, load.short_name);

        case .DIRECTIVE_RUN;
            run := cast(*Code_Directive_Run) node;
            write(p, ifx run.flags & .ASSERTION then "#assert " else "#run ");
            if run.procedure && run.procedure.body_or_null && run.procedure.body_or_null.block {
                statements := run.procedure.body_or_null.block.statements;
                if statements.count == 1 && statements[0].kind == .RETURN {
                    ret := cast(*Code_Return) statements[0];
                    if ret.arguments_unsorted.count == 1 {
                        expression(p, ret.arguments_unsorted[0].expression);
                        return;
                    }
                }
                block(p, run.procedure.body_or_null.block);
            }

        case .DIRECTIVE_CODE;
            write(p, "#code ");
            expression(p, (cast(*Code_Directive_Code) node).expression);

        case .DIRECTIVE_INSERT;
            write(p, "#insert ");
            expression(p, (cast(*Code_Directive_Insert) node).expression);

        case .DIRECTIVE_THIS;          write(p, "#this");
        case .DIRECTIVE_THROUGH;       write(p, "#through");
        case .DIRECTIVE_COMPILE_TIME;  write(p, "#compile_time");
        case .DIRECTIVE_CONTEXT_TYPE;  write(p, "#Context");
        case .PLACEHOLDER;             write(p, "#placeholder");

        case .DIRECTIVE_BYTES;
            write(p, "#bytes ");
            expression(p, (cast(*Code_Directive_Bytes) node).expression);

        case .DIRECTIVE_BAKE;
            bake := cast(*Code_Directive_Bake) node;
            write(p, ifx bake.bake_type == .CONSTANTS_BAKE then "#bake_constants " else "#bake_arguments ");
            expression(p, bake.procedure_call);

        case .DIRECTIVE_MODIFY;
            write(p, "#modify ");
            modify := cast(*Code_Directive_Modify) node;
            if modify.block  block(p, modify.block);

        case .DIRECTIVE_LIBRARY;
            library := cast(*Code_Directive_Library) node;
            write(p, ifx library.library_flags & .IS_SYSTEM_LIBRARY then "#system_library " else "#library ");
            string_literal(p, library.name);

        case .EXPRESSION_QUERY;
            query := cast(*Code_Expression_Query) node;
            if query.query_kind == {
                case .TYPE_OF;     write(p, "type_of(");
                case .IS_CONSTANT; write(p, "is_constant(");
                case .CODE_OF;     write(p, "code_of(");
            }
            expression(p, query.expression_to_query);
            write(p, ")");

        case .TYPE_QUERY;
            query := cast(*Code_Type_Query) node;
            if query.query_kind == {
                case .SIZE_OF;        write(p, "size_of(");
                case .TYPE_INFO;      write(p, "type_info(");
                case .INITIALIZER_OF; write(p, "initializer_of(");
            }
            expression(p, query.type_to_query);
            write(p, ")");

        case .PUSH_CONTEXT;
            push := cast(*Code_Push_Context) node;
            write(p, "push_context ");
            expression(p, push.to_push);
            write(p, " ");
            if push.block  block(p, push.block);

        case .NOTE;
            write(p, "@");
            write(p, (cast(*Code_Note) node).text);

        case .DIRECTIVE_PLACE;
            write(p, "#place ");
            expression(p, (cast(*Code_Directive_Place) node).ident);

        case .DIRECTIVE_SCOPE;
            scope := cast(*Code_Directive_Scope) node;
            if scope.scope_type == {
                case .EXPORT;   write(p, "#scope_export");
                case .FILE;     write(p, "#scope_file");
                case .INTERNAL; write(p, "#scope_module");
            }

        case .DIRECTIVE_LOCATION;
            location := cast(*Code_Directive_Location) node;
            if location.is_caller_location  write(p, "#caller_location");
            else {
                write(p, "#location(");
                expression(p, location.expression);
                write(p, ")");
            }

        case .DIRECTIVE_ADD_CONTEXT;
            write(p, "#add_context ");
            expression(p, (cast(*Code_Directive_Add_Context) node).expression);

        case .DEFER;
            defer_node := cast(*Code_Defer) node;
            write(p, ifx defer_node.is_backticked then "`defer " else "defer ");
            statement(p, defer_node.block);

        case .USING;
            using_node := cast(*Code_Using) node;
            write(p, "using ");
            if using_node.filter_type == {
                case .ONLY;   write(p, ",only(");
                case .EXCEPT; write(p, ",except(");
                case .MAP;    write(p, ",map(");
            }
            if using_node.filter_type != .NONE {
                expression(p, using_node.filter_expression);
                write(p, ") ");
            }
            expression(p, using_node.expression);

        case .DIRECTIVE_PROCEDURE_NAME;
            write(p, "#procedure_name(");
            expression(p, (cast(*Code_Directive_Procedure_Name) node).argument);
            write(p, ")");

        case .DIRECTIVE_EXISTS;
            write(p, "#exists(");
            expression(p, (cast(*Code_Directive_Exists) node).query_expression);
            write(p, ")");

        case .RESOLVED_OVERLOAD;
            expression(p, (cast(*Code_Resolved_Overload) node).source_expression);

        case .ASM;
            write(p, "#asm { }");

        case;
            print_to_builder(p.builder, "/* % */", node.kind);
    }
}
