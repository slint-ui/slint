# Refactor a multi-page UI

## Start with the target structure

For a single-file UI with several screens, build the new page contracts before extracting widgets.
When the public interface may change, replace the old window API as part of the refactor.
Preserve a compatibility adapter only for an explicit caller requirement.

Use the [runnable project](../examples/library/slint-project.json) as the starting skeleton.
Copy its layout into the UI directory or stage it beside the old UI.
Replace the sample pages and commands with the application's own.
For an existing modular UI, use the same conventions in the affected feature.

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

1. Write a page file and its feature global together.
   Define the domain types the host needs. Give closed choices enums and model rows typed fields for identity, label, and image.
   Replace a command such as `action("repeat", "track")` with `Player.change-repeat(RepeatMode.track)`.
   Re-export the global from the page index and main.slint.
2. Move controls and behavior into that page.
   Create dialog instances where they open. A button and a page shortcut both call `confirm.show()`; the popup owns Escape, cancel and confirm.
   Keep draft input, selection awaiting confirmation, and row expansion local.
3. Move reusable visual controls into individual files under components/.
   Keep all page layout in the page. A horizontal layout with a stretching spacer places sidebars over a background.
   Replace repeated measurements at the use sites with named metrics.

## Review the source before rendering

Inspect the implemented API, including every global. For example:

```sh
rg -n -g '*.slint' '^export (component|global|enum)|callback |(in-out|in) property'
```

- A single window or global containing unrelated pages' members means the migration is incomplete. Redistribute them by feature.
- Follow a command from its button to its destination. Remove any string dispatch or integer mode switch along that path.
- Follow one form from opening to dismissal. Replace a central dialog selector with a page-owned PopupWindow. Place its shortcuts in its FocusScope.
- Check the files themselves. Each reusable widget has a home in components/; style.slint and metrics.slint contain values. Replace repeated panel coordinates with layouts.

Build and exercise the finished project in the requested Slint viewer.
