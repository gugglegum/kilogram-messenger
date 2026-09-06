use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use kilogram_offline::{
    PublicationConflictRequestInspection, PublicationConflictResponseInspection,
    authorize_publication_conflict, inspect_publication_conflict_request,
    inspect_publication_conflict_response, render_request_verification_qr,
    render_response_verification_qr,
};

#[derive(Debug, Parser)]
#[command(
    name = "kilogram-offline",
    about = "Minimal offline viewer and Account Root signer",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read-only authentication and display of one Device-signed request.
    InspectRequest {
        #[arg(long)]
        request_file: PathBuf,

        /// Write a new compact exact-match claim as a PNG QR code.
        #[arg(long)]
        qr_output_file: Option<PathBuf>,

        /// Require an existing QR claim to match the signed artifact exactly.
        #[arg(long)]
        verification_qr_file: Option<PathBuf>,
    },

    /// Confirm and Root-sign one exact request; never creates or modifies Root state.
    Authorize {
        /// Existing offline Account Root directory.
        #[arg(long)]
        account_dir: PathBuf,

        #[arg(long)]
        request_file: PathBuf,

        /// Exact KPC1 code independently compared by the operator.
        #[arg(long)]
        confirm_code: String,

        /// Optionally require a QR claim to match before Account Root is loaded.
        #[arg(long)]
        verification_qr_file: Option<PathBuf>,

        /// New no-clobber Root-signed response outside the Root directory.
        #[arg(long)]
        output_file: PathBuf,
    },

    /// Read-only authentication and display of one Root-signed response.
    InspectResponse {
        #[arg(long)]
        response_file: PathBuf,

        /// Write a new compact exact-match claim as a PNG QR code.
        #[arg(long)]
        qr_output_file: Option<PathBuf>,

        /// Require an existing QR claim to match the signed artifact exactly.
        #[arg(long)]
        verification_qr_file: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::InspectRequest {
            request_file,
            qr_output_file,
            verification_qr_file,
        } => {
            let inspected = inspect_publication_conflict_request(
                &request_file,
                verification_qr_file.as_deref(),
            )?;
            print_request(inspected.report());
            if let Some(output) = qr_output_file {
                let report = render_request_verification_qr(&inspected, &output)?;
                print_qr(&output, &report);
            }
            println!("root_secret_loaded=false");
            println!("runtime_state_loaded=false");
            println!("network_surface_compiled=false");
            println!("status=publication-conflict-request-inspected");
        }
        Command::Authorize {
            account_dir,
            request_file,
            confirm_code,
            verification_qr_file,
            output_file,
        } => {
            let report = authorize_publication_conflict(
                &account_dir,
                &request_file,
                &confirm_code,
                verification_qr_file.as_deref(),
                &output_file,
            )?;
            println!(
                "publication_conflict_resolution_request_id={}",
                report.request_id
            );
            println!(
                "publication_conflict_evidence_id={}",
                report.conflict_evidence_id
            );
            println!(
                "publication_conflict_resolution_id={}",
                report.resolution_id
            );
            println!(
                "old_publication_channel_id={}",
                report.old_publication_channel_id
            );
            println!(
                "new_publication_channel_id={}",
                report.new_publication_channel_id
            );
            println!("authority_revision={}", report.authority_revision);
            println!("confirmation_code={}", report.confirmation_code);
            println!("operator_confirmation=exact-match");
            println!("verification_qr_status={}", report.verification_qr_status);
            println!(
                "authorized_at_unix_seconds={}",
                report.authorized_at_unix_seconds
            );
            println!("response_file={}", report.response_file.display());
            println!("root_secret_loaded=true");
            println!("runtime_state_loaded=false");
            println!("network_surface_compiled=false");
            println!("status=publication-conflict-resolution-authorized");
        }
        Command::InspectResponse {
            response_file,
            qr_output_file,
            verification_qr_file,
        } => {
            let inspected = inspect_publication_conflict_response(
                &response_file,
                verification_qr_file.as_deref(),
            )?;
            print_response(inspected.report());
            if let Some(output) = qr_output_file {
                let report = render_response_verification_qr(&inspected, &output)?;
                print_qr(&output, &report);
            }
            println!("root_secret_loaded=false");
            println!("runtime_state_loaded=false");
            println!("network_surface_compiled=false");
            println!("status=publication-conflict-response-inspected");
        }
    }
    Ok(())
}

fn print_request(report: &PublicationConflictRequestInspection) {
    println!("artifact_kind=request");
    println!("request_file={}", report.request_file.display());
    println!("artifact_bytes={}", report.artifact_bytes);
    println!("artifact_digest={}", report.artifact_digest);
    println!(
        "publication_conflict_resolution_request_id={}",
        report.request_id
    );
    println!("local_account_id={}", report.local_account_id);
    println!("requester_device_id={}", report.requester_device_id);
    println!("authority_revision={}", report.authority_revision);
    println!("publication_conflict_proof_id={}", report.conflict_proof_id);
    println!(
        "publication_conflict_evidence_id={}",
        report.conflict_evidence_id
    );
    println!("detector_device_id={}", report.detector_device_id);
    println!(
        "detected_at_unix_seconds={}",
        report.detected_at_unix_seconds
    );
    println!("publication_generation={}", report.publication_generation);
    println!("peer_account_id={}", report.peer_account_id);
    println!("peer_device_id={}", report.peer_device_id);
    println!(
        "old_publication_channel_id={}",
        report.old_publication_channel_id
    );
    println!(
        "new_publication_channel_id={}",
        report.new_publication_channel_id
    );
    println!("old_channel_epoch={}", report.old_channel_epoch);
    println!("new_channel_epoch={}", report.new_channel_epoch);
    println!("route_policy={}", report.route_policy);
    println!(
        "replacement_ticket_digest={}",
        report.replacement_ticket_digest
    );
    println!("replacement_endpoint_id={}", report.replacement_endpoint_id);
    println!(
        "replacement_peer_authority_revision={}",
        report.replacement_peer_authority_revision
    );
    println!(
        "replacement_peer_device_count={}",
        report.replacement_peer_device_count
    );
    println!(
        "requested_at_unix_seconds={}",
        report.requested_at_unix_seconds
    );
    println!("confirmation_code={}", report.confirmation_code);
    println!("verification_qr_status={}", report.verification_qr_status);
    println!("request_signature=valid");
    println!("conflict_evidence_signatures=valid");
    println!("replacement_ticket_signature=valid");
}

fn print_response(report: &PublicationConflictResponseInspection) {
    println!("artifact_kind=response");
    println!("response_file={}", report.response_file.display());
    println!("artifact_bytes={}", report.artifact_bytes);
    println!("artifact_digest={}", report.artifact_digest);
    println!(
        "publication_conflict_resolution_request_id={}",
        report.request_id
    );
    println!(
        "publication_conflict_resolution_id={}",
        report.resolution_id
    );
    println!("local_account_id={}", report.local_account_id);
    println!("authority_revision={}", report.authority_revision);
    println!(
        "publication_conflict_evidence_id={}",
        report.conflict_evidence_id
    );
    println!("peer_account_id={}", report.peer_account_id);
    println!("peer_device_id={}", report.peer_device_id);
    println!(
        "old_publication_channel_id={}",
        report.old_publication_channel_id
    );
    println!(
        "new_publication_channel_id={}",
        report.new_publication_channel_id
    );
    println!("new_channel_epoch={}", report.new_channel_epoch);
    println!("route_policy={}", report.route_policy);
    println!(
        "replacement_ticket_digest={}",
        report.replacement_ticket_digest
    );
    println!("replacement_endpoint_id={}", report.replacement_endpoint_id);
    println!(
        "replacement_peer_authority_revision={}",
        report.replacement_peer_authority_revision
    );
    println!(
        "replacement_peer_device_count={}",
        report.replacement_peer_device_count
    );
    println!(
        "authorized_at_unix_seconds={}",
        report.authorized_at_unix_seconds
    );
    println!("confirmation_code={}", report.confirmation_code);
    println!("verification_qr_status={}", report.verification_qr_status);
    println!("root_resolution_signature=valid");
    println!("replacement_ticket_signature=valid");
}

fn print_qr(path: &std::path::Path, report: &kilogram_offline::QrRenderReport) {
    println!("verification_qr_error_correction=L");
    println!("verification_qr_version={}", report.qr_version);
    println!("verification_qr_module_count={}", report.module_count);
    println!(
        "verification_qr_image_dimensions={}x{}",
        report.pixel_width, report.pixel_height
    );
    println!("verification_qr_png_bytes={}", report.png_bytes);
    println!("verification_qr_file={}", path.display());
}
