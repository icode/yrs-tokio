#[macro_export]
macro_rules! impl_signal_stream_body {
    ($item_pat:pat => $item_expr:expr) => {
        type Item = Result<$crate::signaling::Message, yrs::sync::Error>;

        fn poll_next(
            mut self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Option<Self::Item>> {
            let inner = ::core::pin::Pin::new(&mut self.0);
            match inner.poll_next(cx) {
                ::core::task::Poll::Ready(Some(Ok($item_pat))) => {
                    ::core::task::Poll::Ready(Some(Ok($item_expr)))
                }
                ::core::task::Poll::Ready(Some(Err(e))) => {
                    ::core::task::Poll::Ready(Some(Err(yrs::sync::Error::Other(e.into()))))
                }
                ::core::task::Poll::Ready(None) => ::core::task::Poll::Ready(None),
                ::core::task::Poll::Pending => ::core::task::Poll::Pending,
            }
        }
    };
}

#[macro_export]
macro_rules! impl_yrs_signal_stream {
    // 带 external where 子句的泛型
    ($name:ident<$($gen:ident),+> where $($where_clause:tt)+, $item_pat:pat => $item_expr:expr) => {
        impl<$($gen),+> ::futures::stream::Stream for $name<$($gen),+>
        where
            $($gen: ::tokio::io::AsyncRead + ::tokio::io::AsyncWrite + ::core::marker::Unpin),+,
            $($($where_clause)+)?
        {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };

    // 不带 external where 的泛型
    ($name:ident<$($gen:ident),+>, $item_pat:pat => $item_expr:expr) => {
        impl<$($gen),+> ::futures::stream::Stream for $name<$($gen),+>
        where
            $($gen: ::tokio::io::AsyncRead + ::tokio::io::AsyncWrite + ::core::marker::Unpin),+
        {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };

    // 无泛型参数
    ($name:ident, $item_pat:pat => $item_expr:expr) => {
        impl ::futures::stream::Stream for $name {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };
}

#[macro_export]
macro_rules! to_signaling_message_common {
    ($item:ident, custom $($user_logic:tt)*) => {
        match $item {
            Message::Text(text) => SignalingMessage::Text(text.to_string()),
            Message::Binary(bytes) => SignalingMessage::Binary(bytes.into()),
            Message::Ping(_) => SignalingMessage::Ping,
            Message::Pong(_) => SignalingMessage::Pong,
            Message::Close(_) => SignalingMessage::Close,
            $($user_logic)*
        }
    };
}

#[macro_export]
macro_rules! to_signaling_message {
    ($item:ident, frame) => {
        $crate::to_signaling_message_common!($item, custom Message::Frame(frame) => SignalingMessage::Binary(frame.into_payload().into()),)
    };

    ($item:ident, custom $($user_logic:tt)+) => {
         $crate::to_signaling_message_common!($item, custom $($user_logic)+)
    };

    ($item:ident) => {
        $crate::to_signaling_message_common!($item, custom)
    };
}

#[macro_export]
macro_rules! generate_yrs_poll_fn {
    ($fn_name:ident, $inner_expr:expr) => {
        fn $fn_name(
            mut self: ::core::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            match ::core::pin::Pin::new(&mut $inner_expr).$fn_name(cx) {
                std::task::Poll::Pending => std::task::Poll::Pending,
                std::task::Poll::Ready(Err(e)) => {
                    std::task::Poll::Ready(Err(::yrs::sync::Error::Other(e.into())))
                }
                std::task::Poll::Ready(_) => std::task::Poll::Ready(Ok(())),
            }
        }
    };
}
