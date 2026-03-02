use comrade_lisp::diagnostics::pretty::{PrettyCtx, PrettyPrinter, PrettyDialect, PrettyLimits, PrinterRegistry, PrinterKey};
use comrade_lisp::diagnostics::DiagnosticContext;
use comrade_lisp::proof_state::{ProofState, MorType, LocalContext};
use tower_lsp::lsp_types::Url;

/// Ephemeral pretty-printing context for LSP requests.
///
/// **Lifetime**: Created per hover/inlay/diagnostic request, discarded after.
/// **Design**: Borrowed wrapper avoiding copies of large objects.
pub struct EdgeLordPrettyCtx<'a> {
    registry: &'a PrinterRegistry,
    printer: &'a dyn PrettyPrinter,  // resolved once
    dialect: PrettyDialect,
    limits: PrettyLimits,
    files: &'a DiagnosticContext<'a>,
    proof: &'a ProofState,
    document_uri: &'a Url,
}

impl<'a> EdgeLordPrettyCtx<'a> {
    pub fn new(
        registry: &'a PrinterRegistry,
        dialect: PrettyDialect,
        limits: PrettyLimits,
        proof: &'a ProofState,
        files: &'a DiagnosticContext<'a>,
        document_uri: &'a Url,
    ) -> Self {
        let key = PrinterKey {
            doctrine: None,  // Use global default; future: extract from proof
            dialect,
        };
        let printer = registry.resolve(key);
        
        Self {
            registry,
            printer,
            dialect,
            limits,
            files,
            proof,
            document_uri,
        }
    }

    pub fn document_uri(&self) -> &'a Url {
        self.document_uri
    }

    /// Render a `MorType` using the configured printer dialect.
    ///
    /// Delegates to `PrettyPrinter::render_type`, which formats as `"src → dst"`
    /// with dialect-appropriate arrow style.  The substitution stored in
    /// `self.proof.subst` is applied automatically by the printer.
    ///
    /// **INV D-*:** deterministic.
    pub fn render_mor_type(&self, ty: &MorType) -> String {
        self.printer.render_type(self, ty)
    }

    /// Render a `LocalContext` as a multi-line Markdown-friendly string.
    ///
    /// Each entry is formatted as `"  {name} : {type}"`, one per line.
    /// Returns `"*(empty context)*"` when the context is empty, so hover
    /// output is always non-empty.
    ///
    /// **INV D-*:** entries are in scope order (outermost first) which is
    /// the canonical deterministic order from elaboration.
    pub fn render_local_context(&self, ctx: &LocalContext) -> String {
        if ctx.entries.is_empty() {
            return "*(empty context)*".to_string();
        }
        let mut lines = Vec::new();
        for entry in &ctx.entries {
            let ty_str = match &entry.ty {
                Some(ty) => self.render_mor_type(ty),
                None => "_".to_string(),
            };
            lines.push(format!("  {} : {}", entry.name, ty_str));
        }
        lines.join("\n")
    }
}

impl<'a> PrettyCtx for EdgeLordPrettyCtx<'a> {
    fn printer(&self) -> &dyn PrettyPrinter {
        self.printer
    }
    
    fn proof(&self) -> &ProofState {
        self.proof
    }
    
    fn files(&self) -> &DiagnosticContext<'_> {
        self.files
    }
    
    fn limits(&self) -> PrettyLimits {
        self.limits
    }
}
