// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections.Generic;
using System.Linq;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Slint.SourceGenerator;

/// <summary>Generates the <c>Bind</c> methods of the view models with <c>[SlintViewModel]</c>.</summary>
[Generator]
public sealed class ViewModelBindingGenerator : IIncrementalGenerator
{
    private const string AttributeName = "Slint.SlintViewModelAttribute";
    private const string SlintNameAttribute = "Slint.Generated.SlintNameAttribute";
    private const string ComponentInstance = "Slint.ComponentInstance";

    // CommunityToolkit.Mvvm generates these members in a generator of its own, whose output this one doesn't see.
    private const string ObservablePropertyAttribute = "CommunityToolkit.Mvvm.ComponentModel.ObservablePropertyAttribute";
    private const string RelayCommandAttribute = "CommunityToolkit.Mvvm.Input.RelayCommandAttribute";

    private static readonly DiagnosticDescriptor NotAComponent = new(
        "SLINT101", "Not a generated Slint component",
        "'{0}' isn't a class that Slint generates from a .slint file; pass the class of an exported component to [SlintViewModel]",
        "Slint", DiagnosticSeverity.Error, isEnabledByDefault: true);

    private static readonly DiagnosticDescriptor TypeMismatch = new(
        "SLINT102", "A member of the view model doesn't fit the component",
        "'{0}' of '{1}' has the type '{2}', which doesn't convert to the type '{3}' of the component's '{0}'; bind it with ViewModelBinding.Property and a conversion",
        "Slint", DiagnosticSeverity.Warning, isEnabledByDefault: true);

    private static readonly DiagnosticDescriptor ItemMismatch = new(
        "SLINT103", "The items of a collection differ from the rows of the component's array",
        "The items of '{0}' of '{1}' are '{2}', and the rows of the component's '{0}' are '{3}'; bind it with ViewModelBinding.Collection and a map",
        "Slint", DiagnosticSeverity.Warning, isEnabledByDefault: true);

    public void Initialize(IncrementalGeneratorInitializationContext context)
    {
        var bindings = context.SyntaxProvider.ForAttributeWithMetadataName(
            AttributeName,
            static (node, _) => node is ClassDeclarationSyntax,
            static (context, _) => Generate(context));
        context.RegisterSourceOutput(bindings, static (output, result) =>
        {
            foreach (var diagnostic in result.Diagnostics)
            {
                output.ReportDiagnostic(diagnostic);
            }
            if (result.Source != null)
            {
                output.AddSource(result.HintName, result.Source);
            }
        });
    }

    private sealed class Result(string hintName, string? source, List<Diagnostic> diagnostics)
    {
        public string HintName { get; } = hintName;
        public string? Source { get; } = source;
        public List<Diagnostic> Diagnostics { get; } = diagnostics;
    }

    private sealed class Member(string name, ITypeSymbol type, bool writable)
    {
        public string Name { get; } = name;
        public ITypeSymbol Type { get; } = type;
        public bool Writable { get; } = writable;
    }

    private static Result Generate(GeneratorAttributeSyntaxContext context)
    {
        var viewModel = (INamedTypeSymbol)context.TargetSymbol;
        var compilation = context.SemanticModel.Compilation;
        var diagnostics = new List<Diagnostic>();
        var location = context.TargetNode.GetLocation();
        var methods = new StringBuilder();
        var isPublic = viewModel.DeclaredAccessibility == Accessibility.Public;
        var vmType = viewModel.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat);

        var (properties, commands) = ViewModelMembers(viewModel);
        foreach (var attribute in context.Attributes)
        {
            if (attribute.ConstructorArguments.Length != 1 || attribute.ConstructorArguments[0].Value is not INamedTypeSymbol component)
            {
                continue;
            }
            if (!component.GetMembers("Instance").OfType<IPropertySymbol>().Any(p => p.Type.ToDisplayString() == ComponentInstance))
            {
                diagnostics.Add(Diagnostic.Create(NotAComponent, location, component.ToDisplayString()));
                continue;
            }
            isPublic &= component.DeclaredAccessibility == Accessibility.Public;

            var componentProperties = new Dictionary<string, (string SlintName, IPropertySymbol Property)>();
            var callbacks = new Dictionary<string, string>();
            foreach (var member in component.GetMembers())
            {
                var slintName = member.GetAttributes()
                    .FirstOrDefault(a => a.AttributeClass?.ToDisplayString() == SlintNameAttribute)?
                    .ConstructorArguments.FirstOrDefault().Value as string;
                if (slintName == null)
                {
                    continue;
                }
                if (member is IPropertySymbol property)
                {
                    componentProperties[property.Name] = (slintName, property);
                }
                else if (member is IMethodSymbol method && method.Name.StartsWith("On"))
                {
                    callbacks[method.Name.Substring(2)] = slintName;
                }
            }

            var calls = new StringBuilder();
            var toViewModel = new StringBuilder();
            var enabledProperties = new HashSet<string>();
            foreach (var callback in callbacks)
            {
                if (!commands.Contains(callback.Key + "Command"))
                {
                    continue;
                }
                var enabled = componentProperties.TryGetValue(callback.Key + "Enabled", out var e)
                    && e.Property.Type.SpecialType == SpecialType.System_Boolean && e.Property.SetMethod != null
                    ? $"\"{e.SlintName}\""
                    : "null";
                if (enabled != "null")
                {
                    enabledProperties.Add(callback.Key + "Enabled");
                }
                calls.Append($"\n            .Command(\"{callback.Value}\", static vm => vm.{callback.Key}Command, {enabled})");
            }

            foreach (var entry in componentProperties)
            {
                var name = entry.Key;
                var (slintName, componentProperty) = entry.Value;
                if (enabledProperties.Contains(name) || !properties.TryGetValue(name, out var vmProperty))
                {
                    continue;
                }
                if (componentProperty.Type is INamedTypeSymbol { IsGenericType: true } model
                    && model.OriginalDefinition.ToDisplayString() == "Slint.Model<T>")
                {
                    var row = model.TypeArguments[0];
                    var item = ListItemType(vmProperty.Type);
                    if (item == null)
                    {
                        diagnostics.Add(Diagnostic.Create(TypeMismatch, location, name, viewModel.Name, vmProperty.Type.ToDisplayString(), componentProperty.Type.ToDisplayString()));
                    }
                    else if (!SymbolEqualityComparer.Default.Equals(item, row))
                    {
                        diagnostics.Add(Diagnostic.Create(ItemMismatch, location, name, viewModel.Name, item.ToDisplayString(), row.ToDisplayString()));
                    }
                    else
                    {
                        calls.Append($"\n            .Collection(\"{slintName}\", static vm => vm.{name})");
                    }
                    continue;
                }
                var toComponent = Converts(compilation, vmProperty.Type, componentProperty.Type);
                var backward = vmProperty.Writable && Converts(compilation, componentProperty.Type, vmProperty.Type);
                if (componentProperty.SetMethod != null && toComponent)
                {
                    var setter = backward ? $", static (vm, value) => vm.{name} = value" : "";
                    calls.Append($"\n            .Property(\"{slintName}\", static vm => vm.{name}{setter})");
                }
                else if (componentProperty.SetMethod == null && backward)
                {
                    var type = vmProperty.Type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat);
                    toViewModel.Append($"\n            .ToViewModel<{type}>(\"{slintName}\", static (vm, value) => vm.{name} = value)");
                }
                else if (!toComponent || componentProperty.SetMethod == null)
                {
                    diagnostics.Add(Diagnostic.Create(TypeMismatch, location, name, viewModel.Name, vmProperty.Type.ToDisplayString(), componentProperty.Type.ToDisplayString()));
                }
            }

            var componentType = component.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat);
            methods.Append($$"""

                    /// <summary>Binds the members of the view model to those of the same name of the component.</summary>
                    public static global::Slint.ViewModelBinding<{{vmType}}> Bind(this {{componentType}} component, {{vmType}} viewModel) =>
                        global::Slint.ViewModelBindingExtensions.Bind(component.Instance, viewModel){{calls}}{{toViewModel}};

                """);
        }

        var hintName = viewModel.ToDisplayString().Replace('<', '_').Replace('>', '_') + ".SlintBindings.g.cs";
        if (methods.Length == 0)
        {
            return new Result(hintName, null, diagnostics);
        }
        var ns = viewModel.ContainingNamespace.IsGlobalNamespace ? "" : $"namespace {viewModel.ContainingNamespace.ToDisplayString()};\n\n";
        var source = $$"""
            // <auto-generated/>
            #nullable enable

            {{ns}}/// <summary>The <c>Bind</c> methods of <c>[SlintViewModel]</c> on <see cref="{{vmType}}"/>.</summary>
            {{(isPublic ? "public" : "internal")}} static class {{viewModel.Name}}SlintBindings
            {{{methods}}}
            """;
        return new Result(hintName, source, diagnostics);
    }

    /// <summary>
    /// The properties of the view model and its base classes, with those that <c>[ObservableProperty]</c> generates,
    /// and the names of its commands, with those that <c>[RelayCommand]</c> generates.
    /// </summary>
    private static (Dictionary<string, Member> Properties, HashSet<string> Commands) ViewModelMembers(INamedTypeSymbol viewModel)
    {
        var properties = new Dictionary<string, Member>();
        var commands = new HashSet<string>();
        for (var type = viewModel; type != null && type.SpecialType != SpecialType.System_Object; type = type.BaseType)
        {
            foreach (var member in type.GetMembers())
            {
                if (member.IsStatic)
                {
                    continue;
                }
                switch (member)
                {
                    case IPropertySymbol { DeclaredAccessibility: Accessibility.Public, IsIndexer: false } property:
                        if (IsCommand(property.Type))
                        {
                            commands.Add(property.Name);
                        }
                        else if (!properties.ContainsKey(property.Name))
                        {
                            var writable = property.SetMethod is { DeclaredAccessibility: Accessibility.Public, IsInitOnly: false };
                            properties[property.Name] = new Member(property.Name, property.Type, writable);
                        }
                        break;
                    case IFieldSymbol field when HasAttribute(field, ObservablePropertyAttribute):
                        var name = ObservablePropertyName(field.Name);
                        if (!properties.ContainsKey(name))
                        {
                            properties[name] = new Member(name, field.Type, writable: true);
                        }
                        break;
                    case IMethodSymbol method when HasAttribute(method, RelayCommandAttribute):
                        commands.Add(RelayCommandName(method.Name));
                        break;
                }
            }
        }
        return (properties, commands);
    }

    private static bool HasAttribute(ISymbol symbol, string name) =>
        symbol.GetAttributes().Any(a => a.AttributeClass?.ToDisplayString() == name);

    private static bool IsCommand(ITypeSymbol type) =>
        type.ToDisplayString() == "System.Windows.Input.ICommand"
        || type.AllInterfaces.Any(i => i.ToDisplayString() == "System.Windows.Input.ICommand");

    /// <summary>The property of an <c>[ObservableProperty]</c> field, by the rules of CommunityToolkit.Mvvm: <c>_name</c> and <c>m_name</c> give <c>Name</c>.</summary>
    private static string ObservablePropertyName(string field)
    {
        var name = field.StartsWith("m_") ? field.Substring(2) : field.TrimStart('_');
        return name.Length == 0 ? field : char.ToUpperInvariant(name[0]) + name.Substring(1);
    }

    /// <summary>The command of a <c>[RelayCommand]</c> method, by the rules of CommunityToolkit.Mvvm: <c>OnSaveAsync</c> gives <c>SaveCommand</c>.</summary>
    private static string RelayCommandName(string method)
    {
        var name = method.StartsWith("On") && method.Length > 2 && char.IsUpper(method[2]) ? method.Substring(2) : method;
        if (name.EndsWith("Async") && name.Length > 5)
        {
            name = name.Substring(0, name.Length - 5);
        }
        return name + "Command";
    }

    private static ITypeSymbol? ListItemType(ITypeSymbol type)
    {
        var list = type is INamedTypeSymbol { IsGenericType: true } named && named.OriginalDefinition.ToDisplayString() == "System.Collections.Generic.IList<T>"
            ? named
            : type.AllInterfaces.FirstOrDefault(i => i.OriginalDefinition.ToDisplayString() == "System.Collections.Generic.IList<T>");
        return list?.TypeArguments[0];
    }

    private static bool Converts(Compilation compilation, ITypeSymbol from, ITypeSymbol to) =>
        ((CSharpCompilation)compilation).ClassifyConversion(from, to).IsImplicit || (IsNumber(from) && IsNumber(to));

    private static bool IsNumber(ITypeSymbol type) => type.SpecialType switch
    {
        SpecialType.System_Int32 or SpecialType.System_Int64 or SpecialType.System_Single or SpecialType.System_Double => true,
        _ => false,
    };
}
