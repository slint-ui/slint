# Refactor a multi-page UI

Build one vertical slice first.
Start from the [working Slint 1.19 project](../examples/library/slint-project.json).
Read its [window](../examples/library/main.slint), [page](../examples/library/pages/library.slint), and [popup](../examples/library/components/confirm-removal.slint) together.
Adapt that file structure and ownership to the real application before moving the remaining controls.

```text
main.slint                   Window and typed page selection
slint-project.json           {"main":"main.slint","edition":"1.19"}
style.slint                  Named brushes and colors
metrics.slint                Shared spacing, radii, sizes and typography
pages/{index,library,player}.slint
components/{index,confirm-removal}.slint
assets/
  icons.slint                Icons global referencing icons/*.svg, *.png, etc.
  images.slint               Images global referencing images/*.webp, etc.
  icons/
  images/
```

Create asset groups as needed and keep other media under assets/.
The index files re-export each folder's public names.

## Migrate a page

1. Write the page's host contract beside the page. Give each command a named callback and each closed choice an enum.
   Keep a model item's identity, label and image together in a struct.
   Re-export host-facing globals from main.slint.
2. Move screens one workflow at a time. Give each page its own popup instances and shortcuts.
   Keep draft text in the form and expansion in the row.
   A button and its page's KeyBinding can both call the same popup's show().
   Its confirmed callback calls the feature global directly.
3. Extract reusable controls into individual component files.
   Bind their measurements to shared Metrics properties at the use sites.
   Lay out floating sidebars over a background with a horizontal layout and a stretching spacer.
   Stack panels with a vertical layout.

If the caller permits an interface change, finish replacing the old dispatch API at its source.
The old host can migrate separately.

## Check the result

Read the resulting entry file and globals before compiling.
If one object still owns unrelated pages' models, form fields or commands, move those members to their owning pages.
Replace dialog switches with page-owned PopupWindow instances and raw shortcut comparisons with KeyBinding in the owning FocusScope.
Open the project directory in the requested version of slint-viewer and exercise a shortcut and an open/dismiss cycle.
