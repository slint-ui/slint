export async function startVariableConversion() {
    console.log("Converting variables");

    const currentlySelectedNodes = figma.currentPage.selection;

    console.log("Selected nodes:", currentlySelectedNodes);

    const entireFile = figma.currentPage;

    console.log("Entire file:", entireFile);

    const allLocalPaintStyles = await figma.getLocalPaintStylesAsync();
    console.log("All local styles:", allLocalPaintStyles);

    // These are the top level collections. For example in the OpenBridge file one top collection
    // is called "Set-instrument-digits". This function reutrns an array that includes that and all the others.
    const variableCollections =
        await figma.variables.getLocalVariableCollectionsAsync();
    console.log("Variable collections:", variableCollections);

    // While this returns everything and I am not sure there is any structure to it. 
    const allLocalVariables = await figma.variables.getLocalVariablesAsync();
    console.log("All local variables IDs:", allLocalVariables);
}