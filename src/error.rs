use {
    async_std_resolver::ResolveError,
    aws_sdk_ec2::{
        error::{
            CreateTagsError, DescribeInstancesError, RebootInstancesError, StartInstancesError, StopInstancesError,
            TerminateInstancesError,
        },
        types::SdkError,
    },
    humantime::{DurationError, TimestampError},
    std::{
        error,
        fmt::{Display, Formatter, Result as FmtResult},
    },
};

#[derive(Debug)]
pub(crate) enum Error {
    InvalidDuration(DurationError),
    InvalidTime(TimestampError),
    InvalidUsage(String),
    #[allow(clippy::enum_variant_names)]
    ResolveError(ResolveError),
    #[allow(dead_code)]
    Runtime(String),
    SdkError(Ec2SdkError),
    ShowUsage,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::InvalidDuration(msg) => write!(f, "Invalid duration: {msg}"),
            Self::InvalidTime(msg) => write!(f, "Invalid time: {msg}"),
            Self::InvalidUsage(msg) => write!(f, "Invalid usage: {msg}"),
            Self::ResolveError(e) => write!(f, "DNS error: {e}"),
            Self::Runtime(msg) => write!(f, "Runtime error: {msg}"),
            Self::SdkError(e) => write!(f, "AWS SDK error: {e}"),
            Self::ShowUsage => write!(f, "Show usage"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::InvalidDuration(msg) => Some(msg),
            Self::InvalidTime(msg) => Some(msg),
            Self::InvalidUsage(_) => None,
            Self::ResolveError(e) => Some(e),
            Self::Runtime(_) => None,
            Self::SdkError(e) => Some(e),
            Self::ShowUsage => None,
        }
    }
}

impl From<getopts::Fail> for Error {
    fn from(e: getopts::Fail) -> Self {
        Self::InvalidUsage(e.to_string())
    }
}

impl From<DurationError> for Error {
    fn from(e: DurationError) -> Self {
        Self::InvalidDuration(e)
    }
}

impl From<ResolveError> for Error {
    fn from(e: ResolveError) -> Self {
        Self::ResolveError(e)
    }
}

impl From<TimestampError> for Error {
    fn from(e: TimestampError) -> Self {
        Self::InvalidTime(e)
    }
}

impl From<SdkError<CreateTagsError>> for Error {
    fn from(e: SdkError<CreateTagsError>) -> Self {
        Self::SdkError(e.into())
    }
}

impl From<SdkError<DescribeInstancesError>> for Error {
    fn from(e: SdkError<DescribeInstancesError>) -> Self {
        Self::SdkError(e.into())
    }
}

impl From<SdkError<RebootInstancesError>> for Error {
    fn from(e: SdkError<RebootInstancesError>) -> Self {
        Self::SdkError(e.into())
    }
}

impl From<SdkError<StartInstancesError>> for Error {
    fn from(e: SdkError<StartInstancesError>) -> Self {
        Self::SdkError(e.into())
    }
}

impl From<SdkError<StopInstancesError>> for Error {
    fn from(e: SdkError<StopInstancesError>) -> Self {
        Self::SdkError(e.into())
    }
}

impl From<SdkError<TerminateInstancesError>> for Error {
    fn from(e: SdkError<TerminateInstancesError>) -> Self {
        Self::SdkError(e.into())
    }
}

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Ec2SdkError {
    CreateTagsError(SdkError<CreateTagsError>),
    DescribeInstancesError(SdkError<DescribeInstancesError>),
    RebootInstancesError(SdkError<RebootInstancesError>),
    StartInstancesError(SdkError<StartInstancesError>),
    StopInstancesError(SdkError<StopInstancesError>),
    TerminateInstancesError(SdkError<TerminateInstancesError>),
}

impl Display for Ec2SdkError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::CreateTagsError(e) => write!(f, "Failed to create tags: {e}"),
            Self::DescribeInstancesError(e) => write!(f, "Failed to describe instances: {e}"),
            Self::RebootInstancesError(e) => write!(f, "Failed to reboot instances: {e}"),
            Self::StartInstancesError(e) => write!(f, "Failed to start instances: {e}"),
            Self::StopInstancesError(e) => write!(f, "Failed to stop instances: {e}"),
            Self::TerminateInstancesError(e) => write!(f, "Failed to terminate instances: {e}"),
        }
    }
}

impl error::Error for Ec2SdkError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::CreateTagsError(e) => Some(e),
            Self::DescribeInstancesError(e) => Some(e),
            Self::RebootInstancesError(e) => Some(e),
            Self::StartInstancesError(e) => Some(e),
            Self::StopInstancesError(e) => Some(e),
            Self::TerminateInstancesError(e) => Some(e),
        }
    }
}

impl From<SdkError<CreateTagsError>> for Ec2SdkError {
    fn from(e: SdkError<CreateTagsError>) -> Self {
        Self::CreateTagsError(e)
    }
}

impl From<SdkError<DescribeInstancesError>> for Ec2SdkError {
    fn from(e: SdkError<DescribeInstancesError>) -> Self {
        Self::DescribeInstancesError(e)
    }
}

impl From<SdkError<RebootInstancesError>> for Ec2SdkError {
    fn from(e: SdkError<RebootInstancesError>) -> Self {
        Self::RebootInstancesError(e)
    }
}

impl From<SdkError<StartInstancesError>> for Ec2SdkError {
    fn from(e: SdkError<StartInstancesError>) -> Self {
        Self::StartInstancesError(e)
    }
}

impl From<SdkError<StopInstancesError>> for Ec2SdkError {
    fn from(e: SdkError<StopInstancesError>) -> Self {
        Self::StopInstancesError(e)
    }
}

impl From<SdkError<TerminateInstancesError>> for Ec2SdkError {
    fn from(e: SdkError<TerminateInstancesError>) -> Self {
        Self::TerminateInstancesError(e)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
pub(crate) type NResult = Result<()>;
