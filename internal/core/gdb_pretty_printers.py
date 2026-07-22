# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# GDB pretty printers for types defined in i_slint_core
import gdb
import gdb.printing


class SharedVectorProvider:
    def __init__(self, val):
        self.val = val

    def to_string(self):
        try:
            size = int(self.val["inner"]["pointer"]["header"]["size"])
            return f"<SharedVector, len={size}>"
        except Exception as e:
            return f"<SharedVector: error reading size: {e}>"

    def children(self):
        try:
            size = int(self.val["inner"]["pointer"]["header"]["size"])
            if size == 0:
                return

            inner_struct = self.val["inner"]["pointer"].dereference()
            maybe_uninit = inner_struct["data"]
            elem_type = self._element_type(maybe_uninit)
            data_ptr = maybe_uninit.address.cast(elem_type.pointer())
            for i in range(size):
                yield f"[{i}]", (data_ptr + i).dereference()
        except Exception as e:
            yield "<error>", f"error reading elements: {e}"

    def display_hint(self):
        return "array"

    @staticmethod
    def _element_type(maybe_uninit_val):
        ty = maybe_uninit_val.type
        for field in ty.fields():
            if field.name == "value":
                val_type = field.type
                if val_type.code == gdb.TYPE_CODE_STRUCT:
                    for sub in val_type.fields():
                        if sub.name in ("value", "__0", "0"):
                            return sub.type
                return val_type
        raise RuntimeError(
            "unsupported MaybeUninit layout for SharedVector element type"
        )


class SharedVectorSubPrinter(gdb.printing.SubPrettyPrinter):
    def __init__(self):
        super().__init__("SharedVector")

    def __call__(self, val):
        if not self.enabled:
            return None
        t = val.type.strip_typedefs()
        if t.code == gdb.TYPE_CODE_PTR:  # also support reference to SharedVector
            try:
                val = val.dereference()
                t = val.type.strip_typedefs()
            except gdb.error:
                return None
        if (
            t.code == gdb.TYPE_CODE_STRUCT
            and t.tag
            and t.tag.startswith("i_slint_core::sharedvector::SharedVector<")
        ):
            return SharedVectorProvider(val)
        return None


class SliceProvider:
    def __init__(self, val):
        self.val = val

    def to_string(self):
        try:
            return f"<Slice, len={int(self.val['len'])}>"
        except Exception as e:
            return f"<Slice: error reading len: {e}>"

    def children(self):
        try:
            length = int(self.val["len"])
            if length == 0:
                return

            data_ptr = self._data_pointer()
            elem_type = data_ptr.type.target()
            for i in range(length):
                yield f"[{i}]", (data_ptr + i).dereference()
        except Exception as e:
            yield "<error>", f"error reading elements: {e}"

    def display_hint(self):
        return "array"

    def _data_pointer(self):
        nn = self.val["ptr"]
        if nn.type.code == gdb.TYPE_CODE_PTR:
            return nn
        for field in nn.type.fields():
            if field.name in ("pointer", "__0", "0"):
                candidate = nn[field]
                if candidate.type.code == gdb.TYPE_CODE_PTR:
                    return candidate
        raise RuntimeError("unsupported NonNull layout in Slice")


class SliceSubPrinter(gdb.printing.SubPrettyPrinter):
    def __init__(self):
        super().__init__("Slice")

    def __call__(self, val):
        if not self.enabled:
            return None
        t = val.type.strip_typedefs()
        if t.code == gdb.TYPE_CODE_PTR:  # also support reference to Slice
            try:
                val = val.dereference()
                t = val.type.strip_typedefs()
            except gdb.error:
                return None
        if (
            t.code == gdb.TYPE_CODE_STRUCT
            and t.tag
            and t.tag.startswith("i_slint_core::slice::Slice<")
        ):
            return SliceProvider(val)
        return None


class SlintCorePrettyPrinter(gdb.printing.PrettyPrinter):
    def __init__(self):
        super().__init__("i_slint_core", [])
        self.subprinters = [SharedVectorSubPrinter(), SliceSubPrinter()]

    def __call__(self, val):
        for sp in self.subprinters:
            pp = sp(val)
            if pp is not None:
                return pp
        return None


printer = SlintCorePrettyPrinter()


def register_printers(objfile=None):
    gdb.printing.register_pretty_printer(objfile, printer, replace=True)


register_printers()
