//
// Code_Visit: walking a `Code_Node` tree (**C§5.3**).
//
// `Compiler.get_code_node_children` knows where every kind keeps its
// children; this is the walk on top of it. A visitor is told about every
// node, parents before children, and says whether to go on, to leave this
// node's children out, or to stop altogether.
//

#import "Basic";
#import "Compiler";

Visit_Result :: enum u8 {
    CONTINUE;
    SKIP_CHILDREN;
    STOP;
}

Code_Visitor :: #type (node: *Code_Node, depth: s64, data: *void) -> Visit_Result;

// Depth first, in the order the source wrote things. Returns false if the
// visitor stopped the walk.
visit_code :: (root: *Code_Node, visitor: Code_Visitor, data: *void = null) -> bool {
    return visit_code_at(root, 0, visitor, data);
}

// The nodes under `root`, `root` included, depth first.
get_recursive_nodes_depth_first :: (root: *Code_Node, nodes: *[..] *Code_Node) {
    if !root  return;
    array_add(nodes, root);
    children: [..] *Code_Node;
    get_code_node_children(root, *children);
    for children  get_recursive_nodes_depth_first(it, nodes);
    array_free(children);
}

// Every node under `root` of one kind, cast to what that kind is.
find_nodes :: (root: *Code_Node, $T: Type, kind: Code_Node_Kind) -> [..] *T {
    all: [..] *Code_Node;
    get_recursive_nodes_breadth_first(root, *all);
    found: [..] *T;
    for all  if it.kind == kind  array_add(*found, cast(*T) it);
    array_free(all);
    return found;
}

// Replaces every child pointer of `parent` that is `old` with `new`, which is
// how a metaprogram swaps one node of a tree it was handed for another
// before giving it back. Returns how many it replaced.
replace_child :: (parent: *Code_Node, old: *Code_Node, new: *Code_Node) -> s64 {
    replaced := 0;
    swap :: (slot: **$T, old: *Code_Node, new: *Code_Node) -> s64 #expand {
        if cast(*Code_Node) slot.* != old  return 0;
        slot.* = cast(*T) new;
        return 1;
    }
    if parent.kind == {
        case .BLOCK;
            block := cast(*Code_Block) parent;
            for * block.statements  replaced += swap(it, old, new);
        case .UNARY_OPERATOR;
            replaced += swap(*(cast(*Code_Unary_Operator) parent).subexpression, old, new);
        case .BINARY_OPERATOR;
            binary := cast(*Code_Binary_Operator) parent;
            replaced += swap(*binary.left, old, new);
            replaced += swap(*binary.right, old, new);
        case .PROCEDURE_CALL;
            call := cast(*Code_Procedure_Call) parent;
            replaced += swap(*call.procedure_expression, old, new);
            for * call.arguments_unsorted  replaced += swap(*it.expression, old, new);
        case .IF;
            branch := cast(*Code_If) parent;
            replaced += swap(*branch.condition, old, new);
            replaced += swap(*branch.then_block, old, new);
            replaced += swap(*branch.else_block, old, new);
        case .WHILE;
            loop := cast(*Code_While) parent;
            replaced += swap(*loop.condition, old, new);
            replaced += swap(*loop.block, old, new);
        case .FOR;
            loop := cast(*Code_For) parent;
            replaced += swap(*loop.iteration_expression, old, new);
            replaced += swap(*loop.iteration_expression_right, old, new);
            replaced += swap(*loop.block, old, new);
        case .RETURN;
            ret := cast(*Code_Return) parent;
            for * ret.arguments_unsorted  replaced += swap(*it.expression, old, new);
        case .DECLARATION;
            declaration := cast(*Code_Declaration) parent;
            replaced += swap(*declaration.expression, old, new);
        case .CAST;
            replaced += swap(*(cast(*Code_Cast) parent).expression, old, new);
        case .CASE;
            replaced += swap(*(cast(*Code_Case) parent).condition, old, new);
        case .DEFER;
            replaced += swap(*(cast(*Code_Defer) parent).block, old, new);
        case;
    }
    return replaced;
}

#scope_file

visit_code_at :: (node: *Code_Node, depth: s64, visitor: Code_Visitor, data: *void) -> bool {
    if !node  return true;
    result := visitor(node, depth, data);
    if result == .STOP           return false;
    if result == .SKIP_CHILDREN  return true;
    children: [..] *Code_Node;
    defer array_free(children);
    get_code_node_children(node, *children);
    for children  if !visit_code_at(it, depth + 1, visitor, data)  return false;
    return true;
}
