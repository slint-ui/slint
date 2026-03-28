// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// An image that can be displayed in a Slint UI.
///
/// `SlintImage` wraps Slint's `Image` type, providing access to images loaded
/// from file paths or embedded data. The image data is heap-allocated and
/// managed by the Rust runtime via reference counting.
public final class SlintImage: @unchecked Sendable {
    var handle: OpaquePointer

    /// Creates an image by loading from a file path.
    ///
    /// - Parameter path: The file system path to the image file.
    public init(fromPath path: String) {
        let slintPath = SlintString(path)
        handle = slint_swift_image_load_from_path(&slintPath.handle)
    }

    /// Creates an image by taking ownership of an existing handle.
    init(owning pointer: OpaquePointer) {
        handle = pointer
    }

    /// Creates a copy of another image.
    public init(cloning other: SlintImage) {
        handle = slint_swift_image_clone(other.handle)
    }

    deinit {
        slint_swift_image_drop(handle)
    }

    /// The size of the image in pixels.
    public var size: (width: UInt32, height: UInt32) {
        let s = slint_image_size(handle)
        return (width: s.width, height: s.height)
    }

    /// The file path of the image, if it was loaded from a file.
    public var path: String? {
        guard let pathPtr = slint_image_path(handle) else {
            return nil
        }
        guard let bytes = slint_shared_string_bytes(pathPtr) else {
            return nil
        }
        return String(cString: bytes)
    }

    /// Sets the nine-slice edges for this image.
    ///
    /// Nine-slice scaling divides the image into 9 regions that scale independently,
    /// preserving corners and edges while stretching the center.
    public func setNineSliceEdges(top: UInt16, right: UInt16, bottom: UInt16, left: UInt16) {
        slint_image_set_nine_slice_edges(handle, top, right, bottom, left)
    }
}

extension SlintImage: Equatable {
    public static func == (lhs: SlintImage, rhs: SlintImage) -> Bool {
        slint_image_compare_equal(lhs.handle, rhs.handle)
    }
}
