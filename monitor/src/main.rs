mod score;

use std::sync::{Arc, Mutex};

use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Plot, PlotBounds};

use score::ErrorData;

struct TrustScoreApp {
    error_data: Arc<Mutex<Vec<ErrorData>>>,
    block_number: Arc<Mutex<u32>>,
}

impl TrustScoreApp {
    fn new(error_data: Arc<Mutex<Vec<ErrorData>>>, block_number: Arc<Mutex<u32>>) -> Self {
        Self {
            error_data,
            block_number,
        }
    }
}

impl eframe::App for TrustScoreApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint to keep UI updated
        ctx.request_repaint();

        // Triple the default font size
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(28.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(28.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(36.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(28.0, egui::FontFamily::Monospace),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(20.0, egui::FontFamily::Proportional),
        );
        ctx.set_style(style);

        egui::CentralPanel::default().show(ctx, |ui| {
            let block_num = *self.block_number.lock().unwrap();
            ui.heading(format!("Trust Score Error Analysis - Block #{}", block_num));
            ui.add_space(10.0);

            let data = self.error_data.lock().unwrap().clone();

            if data.is_empty() {
                ui.label("Waiting for data...");
                return;
            }

            // Get available space for the plot
            let available_height = ui.available_height();

            // Create bar chart with custom axis formatter for X-axis labels
            let num_bars = data.len();

            // Clone data for the formatter closure
            let data_for_formatter = data.clone();

            Plot::new("error_plot")
                .legend(Legend::default())
                .show_axes(true)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .height(available_height)
                .x_axis_formatter(move |mark, _range| {
                    let index = mark.value as usize;
                    if index < data_for_formatter.len() {
                        data_for_formatter[index].account_name.clone()
                    } else {
                        String::new()
                    }
                })
                .show(ui, |plot_ui| {
                    // Set fixed Y-axis bounds from 0 to 10
                    let x_min = -0.5;
                    let x_max = num_bars as f64 - 0.5;
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([x_min, 0.0], [x_max, 10.0]));

                    let bars: Vec<Bar> = data
                        .iter()
                        .enumerate()
                        .map(|(i, d)| {
                            Bar::new(i as f64, d.error_value as f64)
                                .width(0.7)
                                .name(&d.account_name)
                        })
                        .collect();

                    let chart = BarChart::new(bars).color(egui::Color32::from_rgb(100, 150, 250));
                    plot_ui.bar_chart(chart);
                });
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv()?;

    // Shared state for error data and block number
    let error_data = Arc::new(Mutex::new(Vec::new()));
    let block_number = Arc::new(Mutex::new(0u32));

    // Clone for the blockchain thread
    let error_data_clone = Arc::clone(&error_data);
    let block_number_clone = Arc::clone(&block_number);

    // Spawn a thread to handle blockchain data fetching
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = score::blockchain_task(error_data_clone, block_number_clone).await {
                eprintln!("Blockchain task error: {}", e);
            }
        });
    });

    // Run the GUI on the main thread
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 700.0])
            .with_title("Trust Score Monitor"),
        ..Default::default()
    };

    eframe::run_native(
        "Trust Score Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(TrustScoreApp::new(error_data, block_number)))),
    )?;

    Ok(())
}
