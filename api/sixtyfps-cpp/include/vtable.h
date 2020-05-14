
template<typename T>
struct VBox {
    T *vtable;
    void *instance;
};

template<typename T>
struct VRef {
    T *vtable;
    void *instance;
};

template<typename T>
struct VRefMut {
    T *vtable;
    void *instance;
}
