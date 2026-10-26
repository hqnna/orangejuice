//
// The children of a `Code_Node`, and the walk built on them (**C§5.3**).
//
// Every pointer a node holds to another node, in the order the source wrote
// them. What is left out is what would visit something twice: a block's
// `members` (its declarations, which are among its statements anyway), a
// call's `arguments_sorted` and `resolved_procedure_expression`, and an
// identifier's `resolved_declaration`, which points somewhere else in the
// program entirely.
//

// Appends the direct children of `node` to `children`, nearest first.
get_code_node_children :: (node: *Code_Node, children: *[..] *Code_Node) {
    if !node  return;

    if node.kind == {
        case .BLOCK;
            block := cast(*Code_Block) node;
            add_all(children, block.statements);

        case .LITERAL;
            literal := cast(*Code_Literal) node;
            if literal.value_type == .STRUCT && literal.struct_literal_info {
                info := literal.struct_literal_info;
                add(children, info.type_expression);
                add_all(children, info.arguments);
            } else if literal.value_type == .ARRAY && literal.array_literal_info {
                info := literal.array_literal_info;
                add(children, info.element_type);
                add(children, info.alignment);
                add_all(children, info.array_members);
            }

        case .UNARY_OPERATOR;
            add(children, (cast(*Code_Unary_Operator) node).subexpression);

        case .BINARY_OPERATOR;
            binary := cast(*Code_Binary_Operator) node;
            add(children, binary.left);
            add(children, binary.right);

        case .PROCEDURE_BODY;
            add(children, (cast(*Code_Procedure_Body) node).block);

        case .PROCEDURE_CALL;
            call := cast(*Code_Procedure_Call) node;
            add(children, call.procedure_expression);
            for call.arguments_unsorted {
                add(children, it.name);
                add(children, it.expression);
            }
            if call.context_modification  add_all(children, call.context_modification.modification_expressions);

        case .WHILE;
            loop := cast(*Code_While) node;
            add(children, loop.condition);
            add(children, loop.block);

        case .IF;
            branch := cast(*Code_If) node;
            add(children, branch.condition);
            add(children, branch.then_block);
            add(children, branch.else_block);

        case .LOOP_CONTROL;
            add(children, (cast(*Code_Loop_Control) node).target_ident);

        case .CASE;
            case_node := cast(*Code_Case) node;
            add(children, case_node.condition);
            add(children, case_node.then_block);

        case .RETURN;
            ret := cast(*Code_Return) node;
            for ret.arguments_unsorted {
                add(children, it.name);
                add(children, it.expression);
            }

        case .FOR;
            loop := cast(*Code_For) node;
            add(children, loop.ident_it);
            add(children, loop.ident_it_index);
            add(children, loop.want_replacement_for_expansion);
            add(children, loop.want_pointer_expression);
            add(children, loop.want_reverse_expression);
            add(children, loop.iteration_expression);
            add(children, loop.iteration_expression_right);
            add(children, loop.block);

        case .TYPE_INSTANTIATION;
            inst := cast(*Code_Type_Instantiation) node;
            add(children, inst.type_valued_expression);
            add(children, inst.must_implement);
            add(children, inst.pointer_to);
            add(children, inst.type_directive_target);
            add(children, inst.array_dimension);
            add(children, inst.array_element_type);

        case .ENUM;
            enumeration := cast(*Code_Enum) node;
            add(children, enumeration.internal_type_inst);
            add(children, enumeration.block);
            add_all(children, enumeration.notes);

        case .PROCEDURE_HEADER;
            header := cast(*Code_Procedure_Header) node;
            add(children, header.constants_block);
            add_all(children, header.arguments);
            add_all(children, header.returns);
            add_all(children, header.modify_directives);
            add_all(children, header.notes);
            add(children, header.body_or_null);

        case .STRUCT;
            structure := cast(*Code_Struct) node;
            add(children, structure.arguments_block);
            add(children, structure.constants_block);
            add_all(children, structure.modify_directives);
            add(children, structure.block);
            add_all(children, structure.notes);

        case .COMMA_SEPARATED_ARGUMENTS;
            for (cast(*Code_Comma_Separated_Arguments) node).arguments  add(children, it.node);

        case .EXTRACT;
            add(children, (cast(*Code_Extract) node).from);

        case .DIRECTIVE_BYTES;
            add(children, (cast(*Code_Directive_Bytes) node).expression);

        case .MAKE_VARARGS;
            varargs := cast(*Code_Make_Varargs) node;
            add(children, varargs.element_type);
            add_all(children, varargs.expressions);

        case .DECLARATION;
            declaration := cast(*Code_Declaration) node;
            add(children, declaration.type_inst);
            add(children, declaration.alignment_expression);
            add(children, declaration.expression);
            add_all(children, declaration.notes);

        case .CAST;
            cast_node := cast(*Code_Cast) node;
            add(children, cast_node.target_type);
            add(children, cast_node.expression);

        case .DIRECTIVE_IMPORT;
            import := cast(*Code_Directive_Import) node;
            add(children, import.module_parameters_call);
            add(children, import.program_parameters_call);

        case .DIRECTIVE_RUN;
            add(children, (cast(*Code_Directive_Run) node).procedure);

        case .DIRECTIVE_CODE;
            add(children, (cast(*Code_Directive_Code) node).expression);

        case .DIRECTIVE_POKE_NAME;
            add(children, (cast(*Code_Directive_Poke_Name) node).module_struct);

        case .DIRECTIVE_BAKE;
            add(children, (cast(*Code_Directive_Bake) node).procedure_call);

        case .DIRECTIVE_MODIFY;
            add(children, (cast(*Code_Directive_Modify) node).block);

        case .EXPRESSION_QUERY;
            add(children, (cast(*Code_Expression_Query) node).expression_to_query);

        case .PUSH_CONTEXT;
            push := cast(*Code_Push_Context) node;
            add(children, push.to_push);
            add(children, push.block);

        case .DIRECTIVE_PLACE;
            add(children, (cast(*Code_Directive_Place) node).ident);

        case .TYPE_QUERY;
            add(children, (cast(*Code_Type_Query) node).type_to_query);

        case .DIRECTIVE_LOCATION;
            add(children, (cast(*Code_Directive_Location) node).expression);

        case .DIRECTIVE_MODULE_PARAMETERS;
            parameters := cast(*Code_Directive_Module_Parameters) node;
            add(children, parameters.module_parameters);
            add(children, parameters.program_parameters);
            add(children, parameters.common_code);

        case .DIRECTIVE_ADD_CONTEXT;
            add(children, (cast(*Code_Directive_Add_Context) node).expression);

        case .COMPOUND_DECLARATION;
            compound := cast(*Code_Compound_Declaration) node;
            add(children, compound.comma_separated_assignment);
            add(children, compound.declaration_properties);
            add(children, compound.alignment_expression);
            add_all(children, compound.notes);

        case .DEFER;
            add(children, (cast(*Code_Defer) node).block);

        case .USING;
            using_node := cast(*Code_Using) node;
            add(children, using_node.expression);
            add(children, using_node.filter_expression);

        case .DIRECTIVE_INSERT;
            insert := cast(*Code_Directive_Insert) node;
            add(children, insert.scope_redirection);
            add(children, insert.break_replacement);
            add(children, insert.continue_replacement);
            add(children, insert.remove_replacement);
            add(children, insert.expression);

        case .DIRECTIVE_PROCEDURE_NAME;
            add(children, (cast(*Code_Directive_Procedure_Name) node).argument);

        case .DIRECTIVE_EXISTS;
            add(children, (cast(*Code_Directive_Exists) node).query_expression);

        case .RESOLVED_OVERLOAD;
            add(children, (cast(*Code_Resolved_Overload) node).source_expression);

        case;
    }
}

// Every node under `node`, `node` itself first, level by level: the
// children of the first node before the grandchildren of any. The export is
// a tree — nothing above follows an identifier to its declaration, or a body
// back to its header — so nothing is listed twice.
get_recursive_nodes_breadth_first :: (node: *Code_Node, nodes: *[..] *Code_Node) {
    if !node  return;
    Basic :: #import "Basic";
    at := nodes.count;
    Basic.array_add(nodes, node);
    while at < nodes.count {
        get_code_node_children(nodes.*[at], nodes);
        at += 1;
    }
}

#scope_file

add :: (children: *[..] *Code_Node, child: *Code_Node) {
    Basic :: #import "Basic";
    if child  Basic.array_add(children, child);
}

add_all :: (children: *[..] *Code_Node, list: [] $T) {
    Basic :: #import "Basic";
    for list  if it  Basic.array_add(children, cast(*Code_Node) it);
}
