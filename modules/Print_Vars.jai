//
// print_vars: prints expressions with the source they were written as.
//
//     x := 42;
//     v := Vector3.{1, 4, 9};
//     print_vars(x, v.y, x + 1);
//
//     x = 42
//     v.y = 4.000000
//     x + 1 = 43
//
// The name comes from the argument's own syntax tree, read at compile time —
// which is the only place it exists, since by runtime an expression is just a
// value. `Code` parameters are what carry it that far.
//
// The reference takes `$args: .. Code`. orangejuice does not support variadic
// `Code` parameters yet (`docs/spec.md` §10.1), so this takes eight defaulted
// ones instead; the call syntax is the same up to eight expressions.
//

#import "Basic";
#import "Compiler";

print_vars :: ($a: Code = #code,null, $b: Code = #code,null, $c: Code = #code,null, $d: Code = #code,null,
               $e: Code = #code,null, $f: Code = #code,null, $g: Code = #code,null, $h: Code = #code,null) #expand {
    #insert -> string {
        return generate("print", a, b, c, d, e, f, g, h);
    }
}

// The same, through `context.logger` rather than straight to stdout.
log_vars :: ($a: Code = #code,null, $b: Code = #code,null, $c: Code = #code,null, $d: Code = #code,null,
             $e: Code = #code,null, $f: Code = #code,null, $g: Code = #code,null, $h: Code = #code,null) #expand {
    #insert -> string {
        return generate("log", a, b, c, d, e, f, g, h);
    }
}

#scope_module

// One `print`/`log` call per argument that was actually given.
generate :: (procedure: string, args: .. Code) -> string {
    builder: String_Builder;
    builder.allocator = temp;

    for args {
        source := render(it);
        if !source.count  continue;
        // The name is written into the format string rather than passed as an
        // argument, so a `%` inside an expression cannot be mistaken for a
        // slot of its own.
        print_to_builder(*builder, "    %(\"", procedure);
        append_escaped(*builder, source);
        print_to_builder(*builder, " = %%\\n\", %);\n", source);
    }
    return builder_to_string(*builder);
}

// The source an expression was written as, reconstructed from its tree. The
// shapes people actually pass to this are names, member accesses, subscripts
// and small arithmetic; anything else comes back as its kind, which is still
// more use than nothing.
render :: (code: Code) -> string {
    root, expressions := compiler_get_nodes(code);
    return render_node(root);
}

render_node :: (node: *Code_Node) -> string {
    if !node  return "";

    if node.kind == cast(u8) Code_Node_Kind.IDENT {
        return (cast(*Code_Ident) node).name;
    }

    if node.kind == cast(u8) Code_Node_Kind.LITERAL {
        literal := cast(*Code_Literal) node;
        if literal.value_type == .STRING  return sprint("\"%\"", literal._string, allocator = temp);
        if literal.value_type == .NUMBER {
            if literal.value_flags & .FLOAT  return sprint("%", literal._float64, allocator = temp);
            return sprint("%", literal._s64, allocator = temp);
        }
        if literal.value_type == .BOOLEAN  return ifx literal._s64 then "true" else "false";
        return "<literal>";
    }

    if node.kind == cast(u8) Code_Node_Kind.UNARY_OPERATOR {
        unary := cast(*Code_Unary_Operator) node;
        return sprint("%1%2", operator_text(unary.operator_type), render_node(unary.subexpression), allocator = temp);
    }

    if node.kind == cast(u8) Code_Node_Kind.BINARY_OPERATOR {
        binary := cast(*Code_Binary_Operator) node;
        left := render_node(binary.left);
        right := render_node(binary.right);

        // A member access reads as `a.b`, without the spaces arithmetic wants.
        if binary.operator_type == cast(Operator_Type) #char "." {
            return sprint("%.%", left, right, allocator = temp);
        }
        if binary.operator_type == .ARRAY_SUBSCRIPT {
            return sprint("%[%]", left, right, allocator = temp);
        }
        return sprint("% % %", left, operator_text(binary.operator_type), right, allocator = temp);
    }

    if node.kind == cast(u8) Code_Node_Kind.PROCEDURE_CALL {
        call := cast(*Code_Procedure_Call) node;
        return sprint("%(...)", render_node(call.procedure_expression), allocator = temp);
    }

    return "<expr>";
}

operator_text :: (kind: Operator_Type) -> string {
    if kind == {
        case .IS_EQUAL;         return "==";
        case .IS_NOT_EQUAL;     return "!=";
        case .LOGICAL_AND;      return "&&";
        case .LOGICAL_OR;       return "||";
        case .LESS_OR_EQUAL;    return "<=";
        case .GREATER_OR_EQUAL; return ">=";
        case .SHIFT_LEFT;       return "<<";
        case .SHIFT_RIGHT;      return ">>";
        case;
    }
    // Everything else is the ASCII character it is (**C§5.3**).
    value := cast(s64) kind;
    if value > 0 && value < 128 {
        one: u8 = cast(u8) value;
        return copy_string(to_string(*one, 1), temp);
    }
    return "?";
}

// The name goes inside a format string, so a quote or a backslash in it would
// end the string early.
append_escaped :: (builder: *String_Builder, text: string) {
    for 0..text.count - 1 {
        c := text[it];
        if c == #char "\"" || c == #char "\\"  append(builder, #char "\\");
        append(builder, c);
    }
}
