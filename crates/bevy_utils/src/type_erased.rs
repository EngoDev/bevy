use std::mem::{ManuallyDrop, MaybeUninit};

struct TypedErasedMeta<UserData> {
    offset: usize,
    user_data: UserData,
}

#[derive(Default)]
pub struct TypeErasedVec<UserData> {
    bytes: Vec<MaybeUninit<u8>>,
    metas: Vec<TypedErasedMeta<UserData>>,
}

impl<UserData> TypeErasedVec<UserData> {
    pub fn new() -> Self {
        Self {
            bytes: vec![],
            metas: vec![],
        }
    }

    /// Push a value onto the vec.
    ///
    /// ## Note
    /// The pushed value will be forgotten as if `[std::mem::forget]` is used.
    /// It is up to the caller to read and drop the value if they desire via `[TypeErasedVec::drain]`.
    #[inline]
    pub fn push<T>(&mut self, value: T, user_data: UserData) {
        let size = std::mem::size_of::<T>();
        let old_len = self.bytes.len();

        self.metas.push(TypedErasedMeta {
            offset: old_len,
            user_data,
        });

        // Use `ManuallyDrop` to forget `value` right away, avoiding
        // any use of it after the `ptr::copy_nonoverlapping`.
        let value = ManuallyDrop::new(value);

        if size > 0 {
            self.bytes.reserve(size);

            // SAFETY: The internal `bytes` vector has enough storage for the
            // value (see the call the `reserve` above), the vector has
            // its length set appropriately and can contain any kind of bytes.
            // In case we're writing a ZST and the `Vec` hasn't allocated yet
            // then `as_mut_ptr` will be a dangling (non null) pointer, and
            // thus valid for ZST writes.
            // Also `value` is forgotten so that  when `apply` is called
            // later, a double `drop` does not occur.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &*value as *const T as *const MaybeUninit<u8>,
                    self.bytes.as_mut_ptr().add(old_len),
                    size,
                );
                self.bytes.set_len(old_len + size);
            }
        }
    }

    /// Calls `func` for each previously pushed value from `[TypedErasedVec::push]`.
    /// The `func` is provided the first byte of the initially pushed data and
    /// the supplied user data.
    /// ## Note
    /// The `*mut MaybeUninit<u8>` may _not_ be aligned, so if you
    /// attempt to cast/read the value back, use `[std::ptr::read_unaligned]`.
    #[inline]
    pub fn drain(&mut self, mut func: impl FnMut(*mut MaybeUninit<u8>, UserData)) {
        // SAFETY: The new len is always 0 when can never be larger than the capacity.
        // And since this essentially 'removes' the initial pushed values, there are no
        // new values being adding the need have been initialized.
        unsafe { self.bytes.set_len(0) };

        for TypedErasedMeta { offset, user_data } in self.metas.drain(..) {
            // SAFETY: This is safe since the calculated byte will point to the beginning of the value
            // pushed from a previous `push` call. Also the pointer will never overflow `isize::MAX`
            // do to the safety guarantees of Vec never allocating more than `isize::MAX` bytes.
            let byte = unsafe { self.bytes.as_mut_ptr().add(offset) };
            func(byte, user_data);
        }
    }

    #[inline]
    pub fn iter_mut(&mut self, mut func: impl FnMut(*mut MaybeUninit<u8>, &UserData)) {
        for TypedErasedMeta { offset, user_data } in &self.metas {
            // SAFETY: This is safe since the calculated byte will point to the beginning of the value
            // pushed from a previous `push` call. Also the pointer will never overflow `isize::MAX`
            // do to the safety guarantees of Vec never allocating more than `isize::MAX` bytes.
            let byte = unsafe { self.bytes.as_mut_ptr().add(*offset) };
            func(byte, user_data);
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.metas.clear();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.metas.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct WithPadding(u8, u16);

    #[cfg(miri)]
    #[test]
    fn test_uninit_bytes() {
        let mut queue = TypeErasedVec::<()>::new();
        queue.push(WithPadding(0, 0), ());
        let _ = format!("{:?}", queue.bytes);
    }
}
