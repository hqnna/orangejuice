//
// `Xresource.h` declares its binding enum and a pointer to it in one
// typedef, `} XrmBinding, *XrmBindingList;`, which the enum emitter does not
// split.
//

XrmBinding :: enum s32 {
    XrmBindTightly;
    XrmBindLoosely;
}
XrmBindingList :: *XrmBinding;
