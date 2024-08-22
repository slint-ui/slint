// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0


import * as vscode from "vscode";
import * as fs from "fs-extra";
import simpleGit from "simple-git";
import * as path from "node:path";

export async function newProject(context: vscode.ExtensionContext) {
    type Language = 'Node (JavaScript/TypeScript)' | 'c++' | 'rust';
    
  const language = await vscode.window.showQuickPick(['Node (JavaScript/TypeScript)', 'c++', 'rust'], {
    placeHolder: "What language do you want to use?"
  });

  if (!language) {
    vscode.window.showErrorMessage("Language selection is required.");
    return;
  }

  let repoUrl: string;
  switch (language as Language) {
    case 'Node (JavaScript/TypeScript)':
      repoUrl = "https://github.com/slint-ui/slint-nodejs-template";
      break;
    case 'c++':
      repoUrl = "https://github.com/slint-ui/slint-cpp-template";
      break;
    case 'rust':
      repoUrl = "https://github.com/slint-ui/slint-rust-template";
      break;
    default:
      vscode.window.showErrorMessage("Invalid language selection.");
      return;
  }


  let workspacePath: string | undefined = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

  if (!workspacePath) {
    const folderUris = await vscode.window.showOpenDialog({
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: "Select Folder"
    });

    if (!folderUris || folderUris.length === 0) {
      vscode.window.showErrorMessage("Please select a folder to place the project in.");
      return;
    }

    workspacePath = folderUris[0].fsPath;
  }

  const projectName = await vscode.window.showInputBox({
    prompt: "Enter the name of the new project",
  });

  if (!projectName) {
    vscode.window.showErrorMessage("Project name is required.");
    return;
  }

  const projectPath = path.join(workspacePath, projectName);

  try {
    await fs.ensureDir(projectPath);

    const git = simpleGit();
    await git.clone(repoUrl, projectPath);

    const vscodeFolderPath = path.join(projectPath, '.vscode');
    await fs.ensureDir(vscodeFolderPath);
    const settingsFilePath = path.join(vscodeFolderPath, 'settings.json');
    const settingsContent = {
      "workbench.startupEditor": "none",
      "workbench.activityBar.visible": true,
      "workbench.sideBar.location": "left",
      "files.autoSave": "off",
      "explorer.openEditors.visible": 0,
    };

    await fs.writeJson(settingsFilePath, settingsContent, { spaces: 2 });

    const uri = vscode.Uri.file(projectPath);
    await vscode.commands.executeCommand("vscode.openFolder", uri, true);
  } catch (err: unknown) {
    const errorMessage = err instanceof Error ? err.message : 'Unknown error';
    vscode.window.showErrorMessage(`Failed to create project: ${errorMessage}`);
  }
}