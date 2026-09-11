//
// Printers a program can hand to `context.print_style.struct_printer`, so that
// a vector prints as `(x, y, z)` and a matrix as its rows rather than as a
// struct's members.
//

// Prints any struct of this module that carries a `coef` array — the matrices
// — as `(row), (row), …`. Returns false for anything else, which is what lets
// it be chained.
print_matrix :: (builder: *String_Builder, any: Any, user_data: *void) -> bool {
    info := any.type;
    if info.type != .STRUCT  return false;

    // `IsMatrixFromMathModule` is the marker every matrix of this module
    // declares; `coef` is the two-dimensional view over its storage.
    is_matrix := false;
    coef: *Type_Info_Struct_Member;
    for * member: (cast(*Type_Info_Struct) info).members {
        if member.name == "IsMatrixFromMathModule"  is_matrix = true;
        if member.name == "coef"                    coef = member;
    }
    if !is_matrix  return false;
    if !coef       return false;

    if coef.type.type != .ARRAY  return false;
    rows := cast(*Type_Info_Array) coef.type;
    if rows.element_type.type != .ARRAY  return false;
    columns := cast(*Type_Info_Array) rows.element_type;

    row_count    := rows.array_count;
    column_count := columns.array_count;
    if row_count < 0 || column_count < 0  return false;

    at := cast(*float) any.value_pointer;
    for row: 0..row_count - 1 {
        append(builder, "(");
        for column: 0..column_count - 1 {
            print_to_builder(builder, "%", at.*);
            if column < column_count - 1  append(builder, ", ");
            at += 1;
        }
        append(builder, ")");
        if row < row_count - 1  append(builder, ", ");
    }
    return true;
}

// The one to install: vectors by name, matrices through `print_matrix`.
math_struct_printer :: (builder: *String_Builder, any: Any, user_data: *void) -> bool {
    info := any.type;

    if info == type_info(Vector2) {
        v := cast(*Vector2) any.value_pointer;
        print_to_builder(builder, "(%, %)", v.x, v.y);
        return true;
    }
    if info == type_info(Vector3) {
        v := cast(*Vector3) any.value_pointer;
        print_to_builder(builder, "(%, %, %)", v.x, v.y, v.z);
        return true;
    }
    if info == type_info(Vector4) {
        v := cast(*Vector4) any.value_pointer;
        print_to_builder(builder, "(%, %, %, %)", v.x, v.y, v.z, v.w);
        return true;
    }

    return (info.type == .STRUCT) && print_matrix(builder, any, user_data);
}

#scope_file

using,only(String_Builder, print_to_builder, append) Basic;
