// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

describe("Smoke test", () => {
    it("passes", () => {
        cy.visit("/");

        cy.get(".preview-container .slint-preview", { timeout: 10000 }); // This is generated last!

        // Other UI elements
        cy.get(".edit-area").get(".monaco-editor-background");
        cy.get(".content.welcome").contains(
            "Welcome to the Slint Online Editor",
        );

        // Menu bar:
        cy.get("#menuBar").contains("Share");
        cy.get("#menuBar").contains("Build");
        cy.get("#menuBar").contains("Demos");
    });
});
