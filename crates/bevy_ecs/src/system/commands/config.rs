use crate::{
    prelude::{FallibleCommand, World},
    system::Command,
};
use bevy_utils::tracing::error;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

#[doc(hidden)]
pub trait AddCommand {
    fn add_command(&mut self, command: impl Command);
}

/// Provides configuration mechanisms in case a command errors.
/// You can specify a custom handler via [`FallibleCommandConfig::on_failure`] or
/// use one of the provided implementations.
///
/// ## Note
/// The default error handler logs the error (via [`error!`]), but does not panic.
pub struct FallibleCommandConfig<'a, C, T>
where
    C: FallibleCommand,
    T: AddCommand,
{
    command: Option<C>,
    inner: &'a mut T,
}

impl<'a, C, T> Deref for FallibleCommandConfig<'a, C, T>
where
    C: FallibleCommand,
    T: AddCommand,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a, C, T> DerefMut for FallibleCommandConfig<'a, C, T>
where
    C: FallibleCommand,
    T: AddCommand,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

/// Builtin command error handlers.
pub struct CommandErrorHandler;

impl CommandErrorHandler {
    /// If the command failed, log the error.
    ///
    /// ## Note
    /// This is the default behavior if no error handler is specified.
    pub fn log<E: Debug>(error: E, _ctx: CommandContext) {
        error!("Commands failed with error: {:?}", error)
    }

    /// If the command failed, [`panic!`] with the error.
    pub fn panic<E: Debug>(error: E, _ctx: CommandContext) {
        panic!("Commands failed with error: {:?}", error)
    }

    /// If the command failed, ignore the error and silently succeed.
    pub fn ignore<E>(_error: E, _ctx: CommandContext) {}
}

pub(crate) struct HandledErrorCommand<C, F>
where
    C: FallibleCommand,
    F: FnOnce(C::Error, CommandContext) + Send + Sync + 'static,
{
    pub(crate) command: C,
    pub(crate) error_handler: F,
}

impl<C, F> Command for HandledErrorCommand<C, F>
where
    C: FallibleCommand,
    F: FnOnce(C::Error, CommandContext) + Send + Sync + 'static,
{
    fn write(self: Box<Self>, world: &mut World) {
        let HandledErrorCommand {
            command,
            error_handler,
        } = *self;

        if let Err(error) = command.try_write(world) {
            error_handler(error, CommandContext { world });
        }
    }
}

#[non_exhaustive]
pub struct CommandContext<'a> {
    pub world: &'a mut World,
}

/// Similar to [`FallibleCommandConfig`] however does not
/// implement [`DerefMut`] nor return `&mut T` of the underlying
/// Commands type.
pub struct FinalFallibleCommandConfig<'a, C, T>
where
    C: FallibleCommand,
    T: AddCommand,
{
    command: Option<C>,
    inner: &'a mut T,
}

macro_rules! impl_fallible_commands {
    ($name:ident, $returnty:ty, $returnfunc:ident) => {
        impl<'a, C, T> $name<'a, C, T>
        where
            C: FallibleCommand,
            C::Error: Debug,
            T: AddCommand,
        {
            #[inline]
            pub(crate) fn new(command: C, inner: &'a mut T) -> Self {
                Self {
                    command: Some(command),
                    inner,
                }
            }

            #[inline]
            #[allow(dead_code)]
            fn return_inner(&mut self) -> &mut T {
                self.inner
            }

            #[inline]
            #[allow(dead_code)]
            fn return_unit(&self) {}
        }

        impl<'a, C, T> $name<'a, C, T>
        where
            C: FallibleCommand,
            C::Error: Debug,
            T: AddCommand,
        {
            /// If the command failed, run the provided `error_handler`.
            ///
            /// ## Note
            /// This is normally used in conjunction with [`CommandErrorHandler`].
            /// However, this can also be used with custom error handlers (e.g. closures).
            ///
            /// # Examples
            /// ```
            /// use bevy_ecs::prelude::*;
            ///
            /// fn system(mut commands: Commands) {
            ///     // built-in error handler
            ///     commands.spawn().insert(42).on_err(CommandErrorHandler::ignore);
            ///
            ///     // custom error handler
            ///     commands.spawn().insert(42).on_err(|error, ctx| {});
            /// }
            /// ```
            pub fn on_err(
                &mut self,
                error_handler: impl FnOnce(C::Error, CommandContext) + Send + Sync + 'static,
            ) -> $returnty {
                let command = self
                    .command
                    .take()
                    .expect("Cannot call `on_err` multiple times for a command error handler.");
                self.inner.add_command(HandledErrorCommand {
                    command,
                    error_handler,
                });
                self.$returnfunc()
            }
        }

        impl<'a, C, T> Drop for $name<'a, C, T>
        where
            C: FallibleCommand,
            T: AddCommand,
        {
            fn drop(&mut self) {
                if self.command.is_some() {
                    self.on_err(CommandErrorHandler::log);
                }
            }
        }
    };
}

impl_fallible_commands!(FinalFallibleCommandConfig, (), return_unit);
impl_fallible_commands!(FallibleCommandConfig, &mut T, return_inner);
