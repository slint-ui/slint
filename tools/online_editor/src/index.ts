// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino permalink

import { CommandRegistry } from "@lumino/commands";
import { DockPanel, Menu, MenuBar, SplitPanel, Widget } from "@lumino/widgets";

import { EditorWidget } from "./editor_widget";
import { PreviewWidget } from "./preview_widget";
import { PropertiesWidget } from "./properties_widget";
import { WelcomeWidget } from "./welcome_widget";

const commands = new CommandRegistry();

function create_build_menu(): Menu {
  const menu = new Menu({ commands });
  menu.title.label = "Build";

  menu.addItem({ command: "slint:compile" });
  menu.addItem({ command: "slint:auto_compile" });

  return menu;
}

function create_demo_menu(editor: EditorWidget): Menu {
  const menu = new Menu({ commands });
  menu.title.label = "Demos";

  for (const demo of editor.known_demos()) {
    const command_name = "slint:set_demo_" + demo[1];
    commands.addCommand(command_name, {
      label: demo[1],
      execute: () => {
        editor.set_demo(demo[0]);
      },
    });
    menu.addItem({ command: command_name });
  }
  return menu;
}

function create_share_menu(): Menu {
  const menu = new Menu({ commands });
  menu.title.label = "Share";

  menu.addItem({ command: "slint:copy_permalink" });

  return menu;
}

function main() {
  const preview = new PreviewWidget();
  const editor = new EditorWidget();
  const properties = new PropertiesWidget();
  const welcome = new WelcomeWidget();

  editor.onRenderRequest = (style, source, url, fetcher) => {
    return preview.render(style, source, url, fetcher);
  };
  editor.onNewPropertyData = (binding_text_provider, p) => {
    properties.set_properties(binding_text_provider, p);
  };

  commands.addCommand("slint:compile", {
    label: "Compile",
    iconClass: "fa fa-hammer",
    mnemonic: 1,
    execute: () => {
      editor.compile();
    },
  });

  commands.addCommand("slint:auto_compile", {
    label: "Automatically Compile on Change",
    mnemonic: 1,
    isToggled: () => {
      return editor.auto_compile;
    },
    execute: () => {
      editor.auto_compile = !editor.auto_compile;
    },
  });

  commands.addCommand("slint:copy_permalink", {
    label: "Copy Permalink to Clipboard",
    iconClass: "fa da-share",
    mnemonic: 1,
    execute: () => {
      const params = new URLSearchParams();
      params.set("snippet", editor.current_editor_content);
      const this_url = new URL(window.location.toString());
      this_url.search = params.toString();
      const share_url = this_url.toString();

      navigator.clipboard.writeText(share_url);
    },
  });

  commands.addKeyBinding({
    keys: ["Accel B"],
    selector: "body",
    command: "slint:compile",
  });

  const menu_bar = new MenuBar();
  menu_bar.id = "menuBar";
  menu_bar.addMenu(create_share_menu());
  menu_bar.addMenu(create_build_menu());
  menu_bar.addMenu(create_demo_menu(editor));

  const dock = new DockPanel();
  dock.addWidget(preview);
  dock.addWidget(welcome, { mode: "split-bottom", ref: preview });
  dock.addWidget(properties, { mode: "tab-after", ref: welcome });

  const main = new SplitPanel({ orientation: "horizontal" });
  main.id = "main";
  main.addWidget(editor);
  main.addWidget(dock);

  window.onresize = () => {
    main.update();
  };

  document.addEventListener("keydown", (event: KeyboardEvent) => {
    commands.processKeydownEvent(event);
  });

  editor.editor_ready.then(() => {
    document.body.getElementsByClassName("loader")[0].remove();
  });

  Widget.attach(menu_bar, document.body);
  Widget.attach(main, document.body);
}

window.onload = main;
