// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

describe("Smoke test", () => {
    it("passes", () => {
        cy.visit("/");

        cy.get(".preview-container .slint-preview", { timeout: 20000 }); // This is generated last!

        // Other UI elements
        cy.get(".edit-area").get(".monaco-editor-background");
        cy.get(".content.welcome").contains(
            "Welcome to SlintPad",
        );

        // Menu bar:
        cy.get("#menuBar").contains("Share");
        cy.get("#menuBar").contains("Build");
        cy.get("#menuBar").contains("Demos");
    });
});
