// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore interactor Mobius vtkgl

#include "vtk_view.h"
#include "gl_state.h"

#include <vtkCamera.h>
#include <vtkInteractorStyleTrackballCamera.h>
#include <vtkLookupTable.h>
#include <vtkOpenGLFramebufferObject.h>
#include <vtkOpenGLState.h>
#include <vtkParametricBoy.h>
#include <vtkParametricKlein.h>
#include <vtkParametricMobius.h>
#include <vtkParametricSuperToroid.h>
#include <vtkParametricTorus.h>
#include <vtkPolyData.h>
#include <vtkProperty.h>
#include <vtkTextureObject.h>

#include <algorithm>
#include <cmath>

VtkView::VtkView()
{
    source->SetUResolution(120);
    source->SetVResolution(120);
    source->SetScalarModeToZ();

    vtkNew<vtkLookupTable> colors;
    colors->SetHueRange(0.6, 0.0);
    colors->SetSaturationRange(0.7, 0.7);
    colors->Build();
    mapper->SetInputConnection(source->GetOutputPort());
    mapper->SetLookupTable(colors);

    actor->SetMapper(mapper);
    actor->GetProperty()->SetEdgeColor(0.1, 0.1, 0.1);
    actor->GetProperty()->SetSpecular(0.3);
    actor->GetProperty()->SetSpecularPower(20);

    renderer->AddActor(actor);
    renderer->GradientBackgroundOn();
    renderer->SetBackground(0.05, 0.06, 0.09);
    renderer->SetBackground2(0.22, 0.25, 0.32);
    // Slint blends the texture with what's below, so make it opaque.
    renderer->SetBackgroundAlpha(1.0);

    set_surface(0);
}

VtkView::~VtkView() = default;

void VtkView::setup()
{
    window = vtkSmartPointer<vtkGenericOpenGLRenderWindow>::New();
    window->SetOwnContext(false);
    // Slint makes its context current before invoking the rendering notifier.
    window->SetIsCurrent(true);
    window->SetMapped(true);
    // Render only from render(), never on VTK's initiative.
    window->SetReadyForRendering(false);
    // Slint shows the display framebuffer's texture, so VTK doesn't need to copy it anywhere.
    window->SetFrameBlitModeToNoBlit();
    window->AddRenderer(renderer);
    window->OpenGLInitContext();

    gl_info_ = std::string(reinterpret_cast<const char *>(glGetString(GL_RENDERER))) + "\nOpenGL "
            + reinterpret_cast<const char *>(glGetString(GL_VERSION));

    interactor = vtkSmartPointer<vtkGenericRenderWindowInteractor>::New();
    interactor->SetRenderWindow(window);
    // The interaction styles render after each event, which must wait for Slint's next frame.
    interactor->EnableRenderOff();
    vtkNew<vtkInteractorStyleTrackballCamera> style;
    interactor->SetInteractorStyle(style);
    interactor->Initialize();
}

slint::Image VtkView::render(int width, int height)
{
    GLStateGuard guard;

    interactor->UpdateSize(width, height);

    // VTK caches the OpenGL state, which Slint changed since the last frame.
    auto state = window->GetState();
    state->Reset();
    // VTK sets this once when it initializes the context and expects it to stay.
    state->vtkglDepthFunc(GL_LEQUAL);
    // VTK expects vertex array 0 to be bound outside its own draw calls,
    // as vtkOpenGLVertexArrayObject::Release() leaves it. Skia leaves its own vertex array bound.
    glBindVertexArray(0);

    window->SetReadyForRendering(true);
    window->Render();
    window->SetReadyForRendering(false);

    auto texture = window->GetDisplayFramebuffer()->GetColorAttachmentAsTextureObject(0);
    return slint::Image::create_from_borrowed_gl_2d_rgba_texture(
            texture->GetHandle(),
            { static_cast<uint32_t>(texture->GetWidth()),
              static_cast<uint32_t>(texture->GetHeight()) },
            slint::Image::BorrowedOpenGLTextureOrigin::BottomLeft);
}

void VtkView::teardown()
{
    GLStateGuard guard;
    window->Finalize();
    window->RemoveRenderer(renderer);
    interactor = nullptr;
    window = nullptr;
}

void VtkView::set_surface(int index)
{
    vtkSmartPointer<vtkParametricFunction> function;
    switch (index) {
    case 1:
        function = vtkSmartPointer<vtkParametricKlein>::New();
        break;
    case 2:
        function = vtkSmartPointer<vtkParametricBoy>::New();
        break;
    case 3:
        function = vtkSmartPointer<vtkParametricMobius>::New();
        break;
    case 4: {
        auto toroid = vtkSmartPointer<vtkParametricSuperToroid>::New();
        toroid->SetN1(0.5);
        toroid->SetN2(3.0);
        function = toroid;
        break;
    }
    default:
        function = vtkSmartPointer<vtkParametricTorus>::New();
        break;
    }
    source->SetParametricFunction(function);
    source->Update();
    mapper->SetScalarRange(source->GetOutput()->GetScalarRange());
    reset_camera();
}

void VtkView::set_wireframe(bool wireframe)
{
    if (wireframe) {
        actor->GetProperty()->SetRepresentationToWireframe();
    } else {
        actor->GetProperty()->SetRepresentationToSurface();
    }
}

void VtkView::set_show_edges(bool show_edges)
{
    actor->GetProperty()->SetEdgeVisibility(show_edges);
}

void VtkView::reset_camera()
{
    auto camera = renderer->GetActiveCamera();
    camera->SetPosition(0, -1, 0.6);
    camera->SetFocalPoint(0, 0, 0);
    camera->SetViewUp(0, 0, 1);
    renderer->ResetCamera();
}

void VtkView::rotate(double degrees)
{
    renderer->GetActiveCamera()->Azimuth(degrees);
    renderer->ResetCameraClippingRange();
}

bool VtkView::pointer_event(const slint::language::PointerEvent &event, float x, float y)
{
    if (!interactor) {
        return false;
    }
    interactor->SetEventInformationFlipY(int(x), int(y), event.modifiers.control,
                                         event.modifiers.shift);

    using slint::language::PointerEventButton;
    using slint::language::PointerEventKind;
    switch (event.kind) {
    case PointerEventKind::Down:
        switch (event.button) {
        case PointerEventButton::Left:
            interactor->LeftButtonPressEvent();
            break;
        case PointerEventButton::Middle:
            interactor->MiddleButtonPressEvent();
            break;
        case PointerEventButton::Right:
            interactor->RightButtonPressEvent();
            break;
        default:
            return false;
        }
        ++pressed_buttons;
        return true;
    case PointerEventKind::Up:
        switch (event.button) {
        case PointerEventButton::Left:
            interactor->LeftButtonReleaseEvent();
            break;
        case PointerEventButton::Middle:
            interactor->MiddleButtonReleaseEvent();
            break;
        case PointerEventButton::Right:
            interactor->RightButtonReleaseEvent();
            break;
        default:
            return false;
        }
        pressed_buttons = std::max(pressed_buttons - 1, 0);
        return true;
    case PointerEventKind::Move:
        interactor->MouseMoveEvent();
        return pressed_buttons > 0;
    case PointerEventKind::Cancel:
        interactor->LeftButtonReleaseEvent();
        interactor->MiddleButtonReleaseEvent();
        interactor->RightButtonReleaseEvent();
        pressed_buttons = 0;
        return true;
    }
    return false;
}

void VtkView::zoom(float delta)
{
    renderer->GetActiveCamera()->Dolly(std::pow(1.002, delta));
    renderer->ResetCameraClippingRange();
}
