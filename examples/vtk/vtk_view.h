// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore interactor

#pragma once

#include <slint.h>

#include <vtkActor.h>
#include <vtkGenericOpenGLRenderWindow.h>
#include <vtkGenericRenderWindowInteractor.h>
#include <vtkNew.h>
#include <vtkParametricFunctionSource.h>
#include <vtkPolyDataMapper.h>
#include <vtkRenderer.h>
#include <vtkSmartPointer.h>

#include <string>

// A VTK scene that renders into an OpenGL texture during Slint's rendering notifier.
// Call setup(), render(), and teardown() only from the notifier, where Slint's context is current.
class VtkView
{
public:
    VtkView();
    ~VtkView();

    void setup();
    // Renders the scene at the given size in physical pixels.
    slint::Image render(int width, int height);
    void teardown();

    // A description of the OpenGL implementation, available after setup().
    std::string gl_info() const { return gl_info_; }

    void set_surface(int index);
    void set_wireframe(bool wireframe);
    void set_show_edges(bool show_edges);
    void reset_camera();
    void rotate(double degrees);

    // Positions are in physical pixels, relative to the top-left of the view.
    // Returns whether the event may have changed the view.
    bool pointer_event(const slint::language::PointerEvent &event, float x, float y);
    void zoom(float delta);

private:
    vtkNew<vtkParametricFunctionSource> source;
    vtkNew<vtkPolyDataMapper> mapper;
    vtkNew<vtkActor> actor;
    vtkNew<vtkRenderer> renderer;

    // VTK's render window wraps Slint's OpenGL context, so it only exists between setup() and
    // teardown().
    vtkSmartPointer<vtkGenericOpenGLRenderWindow> window;
    vtkSmartPointer<vtkGenericRenderWindowInteractor> interactor;
    int pressed_buttons = 0;
    std::string gl_info_;
};
